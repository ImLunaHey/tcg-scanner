use crate::{
    Engine,
    image_access::{selected, worker},
};
use scanner_types::{
    cards::CardGame,
    error::{Result, ScannerError},
    finish::FinishEstimate,
    game_detection::CardGameDetection,
    identification::{CardIdentification, IdentificationState},
    scan::ScanRecord,
};
use std::sync::Arc;
use uuid::Uuid;
impl Engine {
    pub async fn estimate_finish(
        self: &Arc<Self>,
        job: Uuid,
        sequence: u32,
        identification: Uuid,
    ) -> Result<CardIdentification> {
        let permit =
            self.edit_workers.clone().try_acquire_owned().map_err(|_| {
                ScannerError::new("busy", "Two image operations are already pending.")
            })?;
        let engine = self.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let _reservation = engine.submissions.lock().await;
            engine.require_saved_job(job).await?;
            let mut record = engine.scan_status(job)?;
            if !record.state.terminal() {
                return Err(ScannerError::new("busy", "Wait for scanning to finish."));
            }
            if !pokemon(&record, sequence) {
                return Err(ScannerError::new(
                    "unsupportedGame",
                    "Finish assessment requires a recognized Pokemon pair.",
                ));
            }
            let mut current = record
                .identifications()?
                .into_iter()
                .find(|i| {
                    i.request.sequence == sequence
                        && i.request.id == identification
                        && i.state != IdentificationState::Processing
                })
                .ok_or_else(|| {
                    ScannerError::new(
                        "conflict",
                        "A completed, current identification is required. Refresh and retry.",
                    )
                })?;
            let estimate = engine.finish_assessment(&record, &current).await?;
            current
                .extensions
                .insert("finishEstimate".into(), serde_json::json!(estimate));
            record.replace_identification(&current)?;
            let saved = current.clone();
            let engine = engine.clone();
            tokio::task::spawn_blocking(move || {
                engine.database.save(&record)?;
                engine.root.save_manifest(&record)?;
                Ok(saved)
            })
            .await
            .map_err(worker)?
        })
        .await
        .map_err(worker)?
    }
    pub(crate) async fn enrich_finish(
        &self,
        record: &ScanRecord,
        current: &mut CardIdentification,
    ) {
        if !pokemon(record, current.request.sequence) {
            return;
        }
        let Some(evidence) = current.evidence.as_ref() else {
            return;
        };
        let estimate = self
            .finish_assessment(record, current)
            .await
            .unwrap_or_else(|_| scanner_recognition::finish::unavailable(evidence));
        current
            .extensions
            .insert("finishEstimate".into(), serde_json::json!(estimate));
    }
    async fn finish_assessment(
        &self,
        record: &ScanRecord,
        current: &CardIdentification,
    ) -> Result<FinishEstimate> {
        let evidence = current.evidence.as_ref().ok_or_else(|| {
            ScannerError::new(
                "conflict",
                "Identify this saved image before assessing its finish.",
            )
        })?;
        let recognizer = self
            .recognizer
            .as_ref()
            .ok_or_else(|| ScannerError::new("configuration", "Recognition is not configured."))?;
        let source = selected(record, current.request.sequence, current.source_edit_id)?;
        if source.sha256 != current.source_sha256 || source.image_count != 1 {
            return Err(ScannerError::new(
                "storage",
                "Finish assessment requires the unchanged, single-frame identification source.",
            ));
        }
        let (root, snapshot, image) = (self.root.clone(), record.clone(), source.clone());
        tokio::task::spawn_blocking(move || root.verify_image(&snapshot, &image))
            .await
            .map_err(worker)??;
        let preparation = record
            .processed_images
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|e| {
                Some(e.request.id) == current.source_edit_id
                    && e.request.sequence == current.request.sequence
            })
            .and_then(|e| e.card_preparation.as_ref());
        recognizer
            .estimate_finish(
                record,
                source,
                evidence,
                preparation,
                current.request.language,
            )
            .await
    }
}
fn pokemon(record: &ScanRecord, sequence: u32) -> bool {
    CardGameDetection::resolve(record)
        .iter()
        .any(|pair| pair.number == sequence.div_ceil(2) && pair.game == CardGame::Pokemon)
}
