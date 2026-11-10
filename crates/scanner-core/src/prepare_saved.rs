use crate::{
    Engine,
    image_access::{selected, worker},
};
use scanner_types::{
    error::{Result, ScannerError},
    game_detection::CardGameDetection,
    images::ProcessedImage,
    scan::ScanRecord,
};
use std::sync::Arc;
use uuid::Uuid;
impl Engine {
    pub async fn prepare_card(
        self: &Arc<Self>,
        job: Uuid,
        sequence: u32,
        id: Uuid,
    ) -> Result<ProcessedImage> {
        if !(1..=1000).contains(&sequence) {
            return Err(ScannerError::new(
                "invalidParams",
                "Sequence must be 1–1000.",
            ));
        }
        let permit =
            self.edit_workers.clone().try_acquire_owned().map_err(|_| {
                ScannerError::new("busy", "Two library writes are already pending.")
            })?;
        let engine = self.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let _reservation = engine.submissions.lock().await;
            engine.require_saved_job(job).await?;
            let reader = engine.clone();
            let record = tokio::task::spawn_blocking(move || reader.scan_status(job))
                .await
                .map_err(worker)??;
            let edit = match existing(&record, sequence, id)? {
                Some(edit) => edit,
                None => {
                    engine
                        .processor
                        .prepare_with_id(&record, sequence, id)
                        .await?
                }
            };
            if edit.request.id != id
                || edit.request.job_id != job
                || edit.request.sequence != sequence
                || edit.card_preparation.is_none()
            {
                return Err(ScannerError::new(
                    "image",
                    "Processor returned inconsistent preparation metadata.",
                ));
            }
            let publisher = engine.clone();
            tokio::task::spawn_blocking(move || publisher.publish_preparation(record, edit))
                .await
                .map_err(worker)?
        })
        .await
        .map_err(worker)?
    }
    fn publish_preparation(
        &self,
        mut record: ScanRecord,
        edit: ProcessedImage,
    ) -> Result<ProcessedImage> {
        let original = selected(&record, edit.request.sequence, None)?;
        if original.sha256 != edit.source_sha256 {
            return Err(ScannerError::new(
                "conflict",
                "Preparation source checksum changed.",
            ));
        }
        self.root.verify_image(&record, original)?;
        if !record
            .processed_images
            .as_deref()
            .unwrap_or_default()
            .iter()
            .any(|e| e.request.id == edit.request.id)
        {
            record
                .processed_images
                .get_or_insert_with(Vec::new)
                .push(edit.clone());
        }
        self.root.verify_image(&record, &edit.image)?;
        record.warnings.retain(|w| {
            !w.starts_with(&format!(
                "Card preparation failed for image {}:",
                edit.request.sequence
            ))
        });
        record.extensions.insert(
            "cardGames".into(),
            serde_json::json!(CardGameDetection::resolve(&record)),
        );
        self.database.save(&record)?;
        self.root.save_manifest(&record)?;
        Ok(edit)
    }
}

fn existing(record: &ScanRecord, sequence: u32, id: Uuid) -> Result<Option<ProcessedImage>> {
    if !record.state.terminal() {
        return Err(ScannerError::new("busy", "Wait for scanning to finish."));
    }
    let edits = record.processed_images.as_deref().unwrap_or_default();
    if let Some(edit) = edits.iter().find(|e| e.request.id == id) {
        if edit.request.sequence != sequence || edit.card_preparation.is_none() {
            return Err(ScannerError::new(
                "conflict",
                "Image operation ID already has different inputs.",
            ));
        }
        return Ok(Some(edit.clone()));
    }
    if edits
        .iter()
        .filter(|e| e.request.sequence == sequence)
        .count()
        >= 8
    {
        return Err(ScannerError::new(
            "resourceLimit",
            "At most eight saved edits per original are supported.",
        ));
    }
    Ok(None)
}
