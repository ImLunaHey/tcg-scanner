use super::super::support::Harness;
use super::condition::saved_pair;
use scanner_types::{
    cards::CardLanguage,
    error::Result,
    finish::{FinishEstimate, FinishState},
    images::{CardPreparation, OriginalImage},
    recognition::{CardFinish, RecognitionEvidence},
    scan::ScanRecord,
};
use std::sync::Arc;
use uuid::Uuid;
struct Estimator(Option<Arc<Gate>>);
struct Gate {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
#[async_trait::async_trait]
impl scanner_core::backend::CardRecognizer for Estimator {
    async fn recognize(&self, _: &ScanRecord, _: &OriginalImage) -> Result<RecognitionEvidence> {
        Ok(RecognitionEvidence::default())
    }
    async fn estimate_finish(
        &self,
        _: &ScanRecord,
        _: &OriginalImage,
        _: &RecognitionEvidence,
        _: Option<&CardPreparation>,
        _: CardLanguage,
    ) -> Result<FinishEstimate> {
        if let Some(gate) = &self.0 {
            gate.entered.notify_one();
            gate.release.notified().await;
        }
        let mut result = FinishEstimate::new(
            FinishState::Estimated,
            None,
            vec!["Fixture estimate".into()],
            None,
        );
        result.suggested_finish = Some(CardFinish::Normal);
        Ok(result)
    }
}
#[tokio::test]
async fn finish_refresh_preserves_reviews_and_requires_current_unmodified_source() {
    let (h, mut record, mut identity, path) =
        saved_pair(Harness::with_recognizer(Some(Arc::new(Estimator(None))))).await;
    identity.evidence = Some(RecognitionEvidence::default());
    record.replace_identification(&identity).unwrap();
    h.engine.database().save(&record).unwrap();
    let checked = h
        .engine
        .estimate_finish(record.id, 2, identity.request.id)
        .await
        .unwrap();
    assert_eq!(
        checked.extensions["finishEstimate"]["suggestedFinish"],
        "normal"
    );
    assert_eq!(checked.finish, CardFinish::Holo);
    let mut unchanged = serde_json::to_value(&checked).unwrap();
    unchanged.as_object_mut().unwrap().remove("finishEstimate");
    assert_eq!(unchanged, serde_json::to_value(&identity).unwrap());
    assert_eq!(
        h.engine
            .storage()
            .read_manifest(&record)
            .unwrap()
            .identifications()
            .unwrap()[0]
            .extensions,
        checked.extensions
    );
    let repeated = h
        .engine
        .estimate_finish(record.id, 2, identity.request.id)
        .await
        .unwrap();
    assert_eq!(checked.extensions, repeated.extensions);
    assert_eq!(
        h.engine
            .estimate_finish(record.id, 2, Uuid::new_v4())
            .await
            .unwrap_err()
            .kind,
        "conflict"
    );
    std::fs::write(path, b"modified").unwrap();
    assert!(
        h.engine
            .estimate_finish(record.id, 2, identity.request.id)
            .await
            .is_err()
    );
    assert_eq!(
        h.engine
            .card_identification(record.id, 2)
            .await
            .unwrap()
            .unwrap()
            .extensions,
        checked.extensions
    );
    h.engine.shutdown().await;
}

#[tokio::test]
async fn accepted_finish_refresh_survives_disconnection_and_shutdown_waits_for_publication() {
    let gate = Arc::new(Gate {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let (h, mut record, mut identity, _) = saved_pair(Harness::with_recognizer(Some(Arc::new(
        Estimator(Some(gate.clone())),
    ))))
    .await;
    identity.evidence = Some(RecognitionEvidence::default());
    record.replace_identification(&identity).unwrap();
    h.engine.database().save(&record).unwrap();
    let engine = h.engine.clone();
    let job = record.id;
    let id = identity.request.id;
    let request = tokio::spawn(async move { engine.estimate_finish(job, 2, id).await });
    tokio::time::timeout(std::time::Duration::from_secs(3), gate.entered.notified())
        .await
        .unwrap();
    request.abort();
    let engine = h.engine.clone();
    let shutdown = tokio::spawn(async move { engine.shutdown().await });
    gate.release.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(3), shutdown)
        .await
        .unwrap()
        .unwrap();
    let saved = h.engine.scan_status(job).unwrap();
    assert_eq!(
        saved.identifications().unwrap()[0].extensions["finishEstimate"]["suggestedFinish"],
        "normal"
    );
    assert_eq!(
        h.engine
            .storage()
            .read_manifest(&saved)
            .unwrap()
            .identifications()
            .unwrap()[0]
            .finish,
        CardFinish::Holo
    );
}
