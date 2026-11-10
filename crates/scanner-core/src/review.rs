use crate::{
    Engine,
    image_access::{selected, worker},
};
use scanner_types::{
    error::{Result, ScannerError},
    identification::{CardIdentification, IdentificationState, ReviewRequest},
    recognition::{CardCondition, CardFinish},
};
use std::sync::Arc;
use uuid::Uuid;
impl Engine {
    pub async fn card_identification(
        &self,
        job: Uuid,
        sequence: u32,
    ) -> Result<Option<CardIdentification>> {
        let database = self.database.clone();
        tokio::task::spawn_blocking(move || {
            let record = database
                .job(job)?
                .ok_or_else(|| ScannerError::new("scanNotFound", "Unknown scan ID."))?;
            selected(&record, sequence, None)?;
            Ok(record
                .identifications()?
                .into_iter()
                .find(|r| r.request.sequence == sequence))
        })
        .await
        .map_err(worker)?
    }
    pub async fn review_card(
        self: &Arc<Self>,
        request: ReviewRequest,
    ) -> Result<CardIdentification> {
        request.validate()?;
        let permit =
            self.edit_workers.clone().try_acquire_owned().map_err(|_| {
                ScannerError::new("busy", "Two library writes are already pending.")
            })?;
        let engine = self.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let _reservation = engine.submissions.lock().await;
            engine.require_saved_job(request.job_id).await?;
            let worker_engine = engine.clone();
            tokio::task::spawn_blocking(move || worker_engine.review_owned(request))
                .await
                .map_err(worker)?
        })
        .await
        .map_err(worker)?
    }
    pub(crate) async fn require_saved_job(&self, job: Uuid) -> Result<()> {
        let state = self.state.lock().await;
        if state.stopping
            || *self.transfer.borrow() == Some(job)
            || state.active.as_ref().is_some_and(|(id, _)| *id == job)
            || state.queue.iter().any(|w| w.id() == job)
        {
            return Err(ScannerError::new(
                "busy",
                "Wait for scanning, processing and Store transfer to finish.",
            ));
        }
        Ok(())
    }
    fn review_owned(&self, request: ReviewRequest) -> Result<CardIdentification> {
        let mut record = self.scan_status(request.job_id)?;
        if !record.state.terminal() {
            return Err(ScannerError::new("busy", "Wait for scanning to finish."));
        }
        selected(&record, request.sequence, None)?;
        let mut result = record
            .identifications()?
            .into_iter()
            .find(|r| {
                r.request.sequence == request.sequence && r.request.id == request.identification_id
            })
            .ok_or_else(|| {
                ScannerError::new(
                    "conflict",
                    "Identification changed or is missing. Refresh before reviewing.",
                )
            })?;
        if result.state == IdentificationState::Processing {
            return Err(ScannerError::new(
                "busy",
                "Identification is still running.",
            ));
        }
        if let Some(id) = &request.card_id {
            // Only this explicit review path can promote similar artwork into
            // a selected printing. Automatic consumers never see visual hints.
            if let Some(evidence) = result.evidence.as_mut()
                && !evidence.candidates.iter().any(|c| c.id == *id)
                && let Some(visual) = evidence
                    .visual_matches()
                    .into_iter()
                    .find(|v| v.candidate.id == *id)
            {
                evidence.candidates.push(visual.candidate);
            }
            if !result
                .evidence
                .as_ref()
                .is_some_and(|e| e.candidates.iter().any(|c| c.id == *id))
            {
                return Err(ScannerError::new(
                    "invalidParams",
                    "Select one of this identification's candidate IDs, or reject suggestions.",
                ));
            }
            result.state = IdentificationState::Confirmed;
            result.selected_card_id = Some(id.clone());
            result.finish = request.finish;
            result.condition = request.condition;
        } else {
            result.state = IdentificationState::Rejected;
            result.selected_card_id = None;
            result.finish = CardFinish::Unverified;
            result.condition = CardCondition::Unassessed;
        }
        result.reviewed_at = Some(chrono::Utc::now());
        result.catalogue_retry_after = None;
        record.replace_identification(&result)?;
        self.database.save(&record)?;
        self.root.save_manifest(&record)?;
        Ok(result)
    }
}
