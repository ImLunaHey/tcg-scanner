use crate::Engine;
use scanner_types::{
    error::Result,
    identification::{CardIdentification, IdentificationRequest, IdentificationState},
    scan::ScanRecord,
};
impl Engine {
    pub(super) async fn retry_identification(
        self: &std::sync::Arc<Self>,
        request: IdentificationRequest,
    ) -> Result<CardIdentification> {
        let permit = self.edit_workers.clone().try_acquire_owned().map_err(|_| {
            scanner_types::error::ScannerError::new(
                "busy",
                "Two library writes are already pending.",
            )
        })?;
        let engine = self.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let _reservation = engine.submissions.lock().await;
            engine.require_saved_job(request.job_id).await?;
            let worker_engine = engine.clone();
            tokio::task::spawn_blocking(move || {
                let record = worker_engine.scan_status(request.job_id)?;
                let result = record
                    .identifications()?
                    .into_iter()
                    .find(|i| i.request == request)
                    .ok_or_else(super::conflict)?;
                worker_engine.root.save_manifest(&record)?;
                Ok(result)
            })
            .await
            .map_err(crate::image_access::worker)?
        })
        .await
        .map_err(crate::image_access::worker)?
    }
    pub(super) fn persist_pending(
        &self,
        record: &mut ScanRecord,
        pending: &CardIdentification,
    ) -> Result<()> {
        record.replace_identification(pending)?;
        self.database.save(record)?;
        if let Err(error) = self.root.save_manifest(record) {
            let mut failed = pending.clone();
            failed.state = IdentificationState::Failed;
            failed.completed_at = Some(chrono::Utc::now());
            failed.error = Some(error.clone());
            record.replace_identification(&failed)?;
            self.database.save(record)?;
            return Err(error);
        }
        Ok(())
    }
    pub(super) fn publish_identification(&self, result: &CardIdentification) -> Result<()> {
        let mut record = self.scan_status(result.request.job_id)?;
        record.replace_identification(result)?;
        self.database.save(&record)?;
        if self.root.save_manifest(&record).is_err() {
            record.warnings.push("Identification is saved locally, but the disk manifest could not be updated. Retry the same identification request after restoring storage.".into());
            self.database.save(&record)?;
        }
        Ok(())
    }
}
