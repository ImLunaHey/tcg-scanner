use super::support::{Harness, json as response_json};
use scanner_core::backend::CardRecognizer;
use scanner_types::{
    error::{Result, ScannerError},
    identification::IdentificationState,
    images::OriginalImage,
    recognition::RecognitionEvidence,
    scan::ScanRecord,
};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::Notify;
use uuid::Uuid;
#[derive(Default)]
struct Recognizer {
    entered: Notify,
    release: Notify,
    calls: AtomicUsize,
}
#[async_trait::async_trait]
impl CardRecognizer for Recognizer {
    async fn recognize(&self, _: &ScanRecord, _: &OriginalImage) -> Result<RecognitionEvidence> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        self.release.notified().await;
        serde_json::from_value(json!({"lines":[],"candidates":[{"id":"sv09-177","name":"Salamence ex","localID":"177","setID":"sv09","setName":"Journey Together","evidence":["Printed set total 159"]}],"warnings":["Manual confirmation required."],"orientation":0})).map_err(|e|ScannerError::new("test",e.to_string()))
    }
}
#[tokio::test]
async fn accepted_identification_persists_after_response_and_shutdown_waits_for_it() {
    let recognizer = Arc::new(Recognizer::default());
    let h = Harness::with_recognizer(Some(recognizer.clone()));
    let (job, bytes) = h.saved();
    let path = format!("/api/v1/images/{job}:1/identification");
    let id = Uuid::new_v4();
    let response = h.call("POST", &path, json!({"id":id})).await;
    assert_eq!(response.status(), 202);
    let pending = response_json(response).await;
    assert_eq!(pending["state"], "processing");
    recognizer.entered.notified().await;
    let retry = response_json(h.call("POST", &path, json!({"id":id})).await).await;
    assert_eq!(pending, retry);
    assert_eq!(
        h.engine
            .card_identification(job, 1)
            .await
            .unwrap()
            .unwrap()
            .state,
        IdentificationState::Processing
    );
    let shutdown = {
        let engine = h.engine.clone();
        tokio::spawn(async move { engine.shutdown().await })
    };
    tokio::task::yield_now().await;
    assert!(!shutdown.is_finished());
    recognizer.release.notify_one();
    shutdown.await.unwrap();
    let result = h.engine.card_identification(job, 1).await.unwrap().unwrap();
    assert_eq!(result.state, IdentificationState::ReviewRequired);
    assert!(result.selected_card_id.is_none());
    assert_eq!(recognizer.calls.load(Ordering::SeqCst), 1);
    let record = h.engine.scan_status(job).unwrap();
    assert_eq!(
        h.engine
            .storage()
            .read_image(&record, &record.images[0], 1024)
            .unwrap(),
        bytes
    );
    assert_eq!(
        h.engine
            .storage()
            .read_manifest(&record)
            .unwrap()
            .identifications()
            .unwrap()[0]
            .state,
        IdentificationState::ReviewRequired
    );
}
#[tokio::test]
async fn language_conflicts_and_duplicate_work_are_rejected_before_recognition() {
    let recognizer = Arc::new(Recognizer::default());
    let h = Harness::with_recognizer(Some(recognizer.clone()));
    let (job, _) = h.saved();
    let path = format!("/api/v1/images/{job}:1/identification");
    let id = Uuid::new_v4();
    let wrong = response_json(
        h.call("POST", &path, json!({"id":id,"language":"ko"}))
            .await,
    )
    .await;
    assert!(wrong.to_string().contains("invalidParams"));
    assert_eq!(recognizer.calls.load(Ordering::SeqCst), 0);
    let pending = h.call("POST", &path, json!({"id":id})).await;
    assert_eq!(pending.status(), 202);
    recognizer.entered.notified().await;
    let conflicting = response_json(
        h.call("POST", &path, json!({"id":id,"language":"ko"}))
            .await,
    )
    .await;
    assert!(conflicting.to_string().contains("conflict"));
    let busy = response_json(h.call("POST", &path, json!({"id":Uuid::new_v4()})).await).await;
    assert!(busy.to_string().contains("busy"));
    recognizer.release.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while h
            .engine
            .card_identification(job, 1)
            .await
            .unwrap()
            .unwrap()
            .state
            == IdentificationState::Processing
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let complete = response_json(h.call("POST", &path, json!({"id":id})).await).await;
    assert_eq!(complete["state"], "reviewRequired");
    assert_eq!(recognizer.calls.load(Ordering::SeqCst), 1);
    let conflicting =
        response_json(h.call("POST", &path, json!({"id":Uuid::new_v4()})).await).await;
    assert!(conflicting.to_string().contains("conflict"));
    h.engine.shutdown().await;
}
