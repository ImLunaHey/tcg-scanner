use super::{
    ledger::{Baseline, Ledger, PlannedCard},
    plan,
};
use crate::{Engine, image_access::worker};
use scanner_staging::{
    Staging,
    contracts::{CardReply, ImportReply},
    ownership,
};
use scanner_types::{
    error::{Result, ScannerError},
    identification::IdentificationState,
    scan::ScanState,
    staging::TransferState,
};
use std::{collections::BTreeSet, sync::Arc};

impl Engine {
    pub(super) async fn run_transfer(
        self: &Arc<Self>,
        mut ledger: Ledger,
        client: Arc<dyn Staging>,
    ) {
        match self.transfer_cards(&mut ledger, client.as_ref()).await {
            Ok(()) => ledger.status.state = TransferState::Sent,
            Err(error) => {
                ledger.status.state = TransferState::Failed;
                ledger.status.error = Some(error);
            }
        }
        if self.save_transfer(&mut ledger).await.is_err() {
            self.state.lock().await.persistence_error = Some(ScannerError::new(
                "storage",
                "Could not save Store transfer status.",
            ));
        }
        self.transfer.send_replace(None);
    }
    async fn transfer_cards(
        self: &Arc<Self>,
        ledger: &mut Ledger,
        client: &dyn Staging,
    ) -> Result<()> {
        self.validate_transfer(ledger).await?;
        let imported = client.import(&ledger.submission).await?;
        validate_import(ledger, &imported)?;
        ledger.status.review_url = Some(scanner_staging::review_url(
            client.origin(),
            &imported.batch.review_url,
        )?);
        self.save_transfer(ledger).await?;
        for card in ledger.cards.clone() {
            self.validate_transfer(ledger).await?;
            let label = format!("C-{:03}", card.number);
            let mut remote = imported
                .cards
                .iter()
                .find(|r| r.card_number == label)
                .cloned()
                .ok_or_else(response_error)?;
            let input = ledger
                .submission
                .cards
                .iter()
                .find(|i| i.card_number == label)
                .cloned()
                .ok_or_else(response_error)?;
            let baseline = ledger
                .baselines
                .as_deref()
                .unwrap_or_default()
                .iter()
                .find(|b| b.card.number == card.number);
            if !ownership::can_sync(
                &remote,
                &input,
                card.pair(),
                baseline.map(|b| (&b.input, b.card.pair())),
            ) {
                ledger
                    .status
                    .preserved_cards
                    .get_or_insert_with(Vec::new)
                    .push(label);
                self.save_transfer(ledger).await?;
                continue;
            }
            if !ownership::metadata_matches(&remote, &input) {
                self.validate_transfer(ledger).await?;
                let updated = client.update(&remote, &input).await?;
                if !same_card(&remote, &updated)
                    || updated.status != "pending"
                    || !ownership::metadata_matches(&updated, &input)
                {
                    return Err(response_error());
                }
                remote = updated;
            }
            if !ownership::photos_match(&remote, card.pair()) {
                let (front, back) = self.transfer_photos(ledger, &card).await?;
                self.validate_transfer(ledger).await?;
                let uploaded = client.upload(&remote, front, back).await?;
                if !same_card(&remote, &uploaded)
                    || !ownership::photos_match(&uploaded, card.pair())
                {
                    return Err(response_error());
                }
            }
            let baselines = ledger.baselines.get_or_insert_with(Vec::new);
            baselines.retain(|b| b.card.number != card.number);
            baselines.push(Baseline { input, card });
            ledger.status.sent_cards += 1;
            self.save_transfer(ledger).await?;
        }
        Ok(())
    }
    async fn validate_transfer(&self, ledger: &Ledger) -> Result<()> {
        {
            let state = self.state.lock().await;
            if state.stopping || state.paused || state.persistence_error.is_some() {
                return Err(ScannerError::new(
                    "storeInterrupted",
                    "Transfer stopped. Retry sending; existing Store cards are retained.",
                ));
            }
        }
        let current = self.scan_status(ledger.snapshot.id)?;
        if current.state != ScanState::Completed
            || current
                .identifications()?
                .iter()
                .any(|i| i.state == IdentificationState::Processing)
        {
            return Err(plan::not_ready(
                "Wait for the batch's image and edition checks.",
            ));
        }
        plan::validate_editions(&current)
    }
    async fn transfer_photos(
        &self,
        ledger: &Ledger,
        card: &PlannedCard,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        let (root, record, card) = (self.root.clone(), ledger.snapshot.clone(), card.clone());
        tokio::task::spawn_blocking(move || {
            // Capability reads validate both the saved length and full SHA-256.
            let front = root.read_image(&record, &card.front, 10 * 1024 * 1024)?;
            let back = root.read_image(&record, &card.back, 10 * 1024 * 1024)?;
            if [&front, &back]
                .iter()
                .any(|b| !b.starts_with(b"\x89PNG\r\n\x1a\n"))
            {
                return Err(ScannerError::new(
                    "integrity",
                    "Prepared PNG verification failed. Nothing was overwritten.",
                ));
            }
            Ok((front, back))
        })
        .await
        .map_err(worker)?
    }
}
fn same_card(before: &CardReply, after: &CardReply) -> bool {
    before.id == after.id
        && before.scan_batch_id == after.scan_batch_id
        && before.card_number == after.card_number
}
fn validate_import(ledger: &Ledger, reply: &ImportReply) -> Result<()> {
    let labels: BTreeSet<_> = ledger
        .submission
        .cards
        .iter()
        .map(|c| c.card_number.as_str())
        .collect();
    if reply.batch.scanner_instance_id != ledger.submission.scanner_instance_id
        || reply.batch.source_batch_id != ledger.snapshot.id
        || reply.batch.batch_number != ledger.submission.batch_number
        || labels != reply.cards.iter().map(|c| c.card_number.as_str()).collect()
        || reply.cards.len() != ledger.cards.len()
        || reply
            .cards
            .iter()
            .map(|c| c.id)
            .collect::<BTreeSet<_>>()
            .len()
            != ledger.cards.len()
        || reply
            .cards
            .iter()
            .any(|c| c.scan_batch_id != reply.batch.id)
    {
        return Err(response_error());
    }
    Ok(())
}
fn response_error() -> ScannerError {
    ScannerError::new(
        "storeResponse",
        "Store did not confirm the expected batch, card metadata or paired photo checksums.",
    )
}
