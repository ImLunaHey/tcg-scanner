use crate::{Engine, runner::Work};
use scanner_types::{
    error::Result,
    game_detection::CardGameDetection,
    identification::{CardIdentification, IdentificationRequest},
};
use tokio_util::sync::CancellationToken;
impl Engine {
    pub(crate) async fn identify_captured(
        &self,
        work: &mut Work,
        sequences: &[u32],
        cancel: &CancellationToken,
    ) -> Result<()> {
        let pairs = CardGameDetection::resolve(work.store.record());
        self.transition(work, |record| {
            record
                .extensions
                .insert("cardGames".into(), serde_json::json!(pairs));
        })?;
        if self.recognizer.is_none() {
            return Ok(());
        }
        let _permit = self
            .recognition_workers
            .acquire()
            .await
            .map_err(crate::image_access::worker)?;
        for &sequence in sequences {
            if cancel.is_cancelled() {
                break;
            }
            let snapshot = work.store.record().clone();
            let original = crate::image_access::selected(&snapshot, sequence, None)?;
            let prepared = snapshot
                .processed_images
                .as_deref()
                .unwrap_or_default()
                .iter()
                .rev()
                .find(|e| e.request.sequence == sequence && e.card_preparation.is_some());
            let image = prepared.map(|e| &e.image).unwrap_or(original);
            let request = IdentificationRequest {
                id: uuid::Uuid::new_v4(),
                job_id: snapshot.id,
                sequence,
                language: snapshot.request.card_language.unwrap_or_default(),
                extensions: Default::default(),
            };
            let pending = CardIdentification::pending(
                request,
                image.sha256.clone(),
                prepared.map(|e| e.request.id),
            );
            self.save_captured_identification(work, &pending)?;
            let result = self.recognize(pending, &snapshot, image).await;
            self.save_captured_identification(work, &result)?;
        }
        Ok(())
    }
    fn save_captured_identification(
        &self,
        work: &mut Work,
        result: &CardIdentification,
    ) -> Result<()> {
        let mut next = work.store.record().clone();
        next.replace_identification(result)?;
        self.transition(work, |record| *record = next)
    }
}
