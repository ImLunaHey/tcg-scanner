use super::{
    ledger::{Baseline, Ledger},
    plan,
};
use crate::{Engine, image_access::worker};
use scanner_staging::Staging;
use scanner_types::{
    error::{Result, ScannerError},
    identification::IdentificationState,
    scan::ScanState,
    staging::{TransferState, TransferStatus},
};
use std::sync::Arc;
use uuid::Uuid;

impl Engine {
    pub fn configure_staging(&self, client: Arc<dyn Staging>) -> Result<()> {
        self.staging
            .set(client)
            .map_err(|_| ScannerError::new("configuration", "Store is already configured."))
    }
    pub async fn store_transfer_status(self: &Arc<Self>, id: Uuid) -> Result<TransferStatus> {
        let engine = self.clone();
        tokio::task::spawn_blocking(move || {
            engine.scan_status(id)?;
            let mut status = engine
                .saved_transfer(id)?
                .map(|v| v.status)
                .unwrap_or_else(|| TransferStatus::new(id, engine.staging.get().is_some()));
            status.configured = engine.staging.get().is_some();
            Ok(status)
        })
        .await
        .map_err(worker)?
    }
    /// Returns persisted progress once accepted. The owned worker survives callers.
    pub async fn send_to_store(self: &Arc<Self>, id: Uuid) -> Result<TransferStatus> {
        let permit =
            self.edit_workers.clone().try_acquire_owned().map_err(|_| {
                ScannerError::new("busy", "Two library writes are already pending.")
            })?;
        let engine = self.clone();
        tokio::spawn(async move {
            let _permit = permit;
            engine.prepare_transfer(id).await
        })
        .await
        .map_err(worker)?
    }
    async fn prepare_transfer(self: &Arc<Self>, id: Uuid) -> Result<TransferStatus> {
        let _reservation = self.submissions.lock().await;
        self.state.lock().await.accepting()?;
        let client = self.staging.get().cloned().ok_or_else(|| {
            ScannerError::new(
                "storeNotConfigured",
                "Store integration is not configured on the scanner service.",
            )
        })?;
        let saved = self.saved_transfer(id)?;
        if *self.transfer.borrow() == Some(id) {
            return saved.map(|v| v.status).ok_or_else(|| {
                ScannerError::new("storage", "Active Store transfer receipt is missing.")
            });
        }
        if self.transfer.borrow().is_some() {
            return Err(ScannerError::new(
                "busy",
                "Another batch is being sent to Store.",
            ));
        }
        self.require_saved_job(id).await?;
        let record = self.scan_status(id)?;
        if record.state != ScanState::Completed
            || record
                .identifications()?
                .iter()
                .any(|i| i.state == IdentificationState::Processing)
        {
            return Err(plan::not_ready(
                "Wait for this batch's image and edition checks before sending.",
            ));
        }
        plan::validate_editions(&record)?;
        if let Some(saved) = &saved {
            same_configuration(saved, client.as_ref())?;
        }
        let mut ledger = match saved {
            Some(ledger) if ledger.status.state != TransferState::Sent => ledger,
            saved => {
                let valuation = self.valuation_for_record(&record).await?;
                let mut ledger = Ledger::make(
                    record,
                    client.origin().into(),
                    client.scanner_instance_id().into(),
                    Some(&valuation),
                )?;
                if let Some(saved) = saved {
                    carry_baselines(&mut ledger, saved)?;
                }
                ledger
            }
        };
        if ledger
            .submission
            .cards
            .iter()
            .any(|c| c.game.is_none() || c.language.is_none())
        {
            upgrade(&mut ledger, client.as_ref())?;
        }
        same_configuration(&ledger, client.as_ref())?;
        plan::validate_editions(&ledger.snapshot)?;
        self.state.lock().await.accepting()?;
        ledger.status.state = TransferState::Sending;
        ledger.status.sent_cards = 0;
        ledger.status.preserved_cards = Some(vec![]);
        ledger.status.error = None;
        self.save_transfer(&mut ledger).await?;
        let status = ledger.status.clone();
        self.transfer.send_replace(Some(id));
        let engine = self.clone();
        tokio::spawn(async move {
            engine.run_transfer(ledger, client).await;
        });
        Ok(status)
    }
    fn saved_transfer(&self, id: Uuid) -> Result<Option<Ledger>> {
        self.database
            .store_transfer(id)?
            .map(|value| {
                let ledger: Ledger = serde_json::from_value(value).map_err(|_| {
                    ScannerError::new("storage", "Store transfer receipt is unreadable.")
                })?;
                ledger.validate(id)?;
                Ok(ledger)
            })
            .transpose()
    }
    pub(super) async fn save_transfer(&self, ledger: &mut Ledger) -> Result<()> {
        ledger.status.updated_at = chrono::Utc::now();
        let ledger = ledger.clone();
        let database = self.database.clone();
        tokio::task::spawn_blocking(move || persist(&database, &ledger))
            .await
            .map_err(worker)?
    }
}
fn same_configuration(ledger: &Ledger, client: &dyn Staging) -> Result<()> {
    if ledger.origin != client.origin()
        || ledger.submission.scanner_instance_id != client.scanner_instance_id()
    {
        return Err(ScannerError::new(
            "storeConflict",
            "This transfer belongs to another Store configuration. Restore the original connection before retrying.",
        ));
    }
    Ok(())
}
fn carry_baselines(ledger: &mut Ledger, saved: Ledger) -> Result<()> {
    if ledger.submission.batch_number != saved.submission.batch_number
        || ledger
            .cards
            .iter()
            .map(|c| c.number)
            .ne(saved.cards.iter().map(|c| c.number))
    {
        return Err(ScannerError::new(
            "storeConflict",
            "Batch/card identities changed since the previous transfer.",
        ));
    }
    ledger.status.review_url = saved.status.review_url;
    ledger.baselines = Some(saved.baselines.unwrap_or_else(|| {
        saved
            .submission
            .cards
            .into_iter()
            .zip(saved.cards)
            .map(|(input, card)| Baseline { input, card })
            .collect()
    }));
    Ok(())
}
fn upgrade(ledger: &mut Ledger, client: &dyn Staging) -> Result<()> {
    let mut fresh = Ledger::make(
        ledger.snapshot.clone(),
        client.origin().into(),
        client.scanner_instance_id().into(),
        None,
    )?;
    if fresh.submission.cards.len() != ledger.submission.cards.len() {
        return Err(plan::not_ready(
            "Stored transfer card count is inconsistent.",
        ));
    }
    for (old, new) in ledger
        .submission
        .cards
        .iter()
        .zip(&mut fresh.submission.cards)
    {
        if old.card_number != new.card_number {
            return Err(plan::not_ready("Stored transfer order is inconsistent."));
        }
        new.review.suggested_price_gbp = old.review.suggested_price_gbp;
    }
    ledger.submission.cards = fresh.submission.cards;
    Ok(())
}
fn persist(database: &scanner_store::Database, ledger: &Ledger) -> Result<()> {
    ledger.validate(ledger.status.job_id)?;
    let value = serde_json::to_value(ledger)
        .map_err(|_| ScannerError::new("storage", "Could not encode Store receipt."))?;
    let status = serde_json::to_value(&ledger.status)
        .map_err(|_| ScannerError::new("storage", "Could not encode Store status."))?;
    database.save_store_transfer(
        ledger.status.job_id,
        ledger.status.state.as_str(),
        &value,
        &status,
    )
}
pub(crate) fn recover(database: &scanner_store::Database) -> Result<()> {
    for value in database.interrupted_store_transfers()? {
        let mut ledger: Ledger = serde_json::from_value(value).map_err(|_| {
            ScannerError::new("storage", "Interrupted Store receipt is unreadable.")
        })?;
        ledger.status.state = TransferState::Failed;
        ledger.status.error = Some(ScannerError::new(
            "storeInterrupted",
            "Transfer stopped. Retry sending; existing Store cards are retained.",
        ));
        ledger.status.updated_at = chrono::Utc::now();
        persist(database, &ledger)?;
    }
    Ok(())
}
