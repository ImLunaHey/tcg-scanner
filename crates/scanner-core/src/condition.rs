use crate::{
    Engine,
    image_access::{selected, worker},
};
use scanner_files::StorageRoot;
use scanner_types::{
    cards::CardGame,
    condition::{ConditionCheck, ConditionState},
    error::{Result, ScannerError},
    game_detection::CardGameDetection,
    identification::{CardIdentification, IdentificationState},
    scan::ScanRecord,
};
use std::sync::Arc;
use uuid::Uuid;

impl Engine {
    pub async fn check_condition(
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
            let engine = engine.clone();
            tokio::task::spawn_blocking(move || {
                engine.check_condition_owned(job, sequence, identification)
            })
            .await
            .map_err(worker)?
        })
        .await
        .map_err(worker)?
    }
    fn check_condition_owned(
        &self,
        job: Uuid,
        sequence: u32,
        identification: Uuid,
    ) -> Result<CardIdentification> {
        let mut record = self.scan_status(job)?;
        if !record.state.terminal() {
            return Err(ScannerError::new("busy", "Wait for scanning to finish."));
        }
        if !pokemon(&record, sequence) {
            return Err(ScannerError::new(
                "unsupportedGame",
                "This check requires a recognized Pokémon card pair.",
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
        let result = analyze(&self.root, &record, &current)?;
        // The submissions reservation serializes source changes and review writes.
        // Only the observation is replaced; identity and manual assessments survive.
        current
            .extensions
            .insert("conditionCheck".into(), serde_json::json!(result));
        record.replace_identification(&current)?;
        self.database.save(&record)?;
        self.root.save_manifest(&record)?;
        Ok(current)
    }
    pub(crate) async fn enrich_condition(
        &self,
        record: &ScanRecord,
        current: &mut CardIdentification,
    ) {
        if !pokemon(record, current.request.sequence) {
            return;
        }
        let (root, record, identity) = (self.root.clone(), record.clone(), current.clone());
        let result = tokio::task::spawn_blocking(move || analyze(&root, &record, &identity))
            .await.map_err(worker).and_then(|v| v).unwrap_or_else(|_| ConditionCheck::new(
                ConditionState::Inconclusive, vec!["The exact saved source could not be analyzed. No condition grade was assigned.".into()]
            ));
        current
            .extensions
            .insert("conditionCheck".into(), serde_json::json!(result));
    }
}
fn pokemon(record: &ScanRecord, sequence: u32) -> bool {
    CardGameDetection::resolve(record)
        .iter()
        .any(|pair| pair.number == sequence.div_ceil(2) && pair.game == CardGame::Pokemon)
}
fn analyze(
    root: &StorageRoot,
    record: &ScanRecord,
    current: &CardIdentification,
) -> Result<ConditionCheck> {
    let source = selected(record, current.request.sequence, current.source_edit_id)?;
    if source.sha256 != current.source_sha256 {
        return Err(ScannerError::new(
            "storage",
            "The identification source image is missing or changed.",
        ));
    }
    let bytes = root.read_image(record, source, 32 * 1024 * 1024)?;
    if source.image_count != 1 {
        return Ok(ConditionCheck::new(
            ConditionState::Inconclusive,
            vec!["Condition analysis requires exactly one image frame.".into()],
        ));
    }
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
    Ok(scanner_recognition::condition::check(&bytes, preparation))
}
