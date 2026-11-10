use crate::support::Harness;
use chrono::Utc;
use scanner_core::backend::CardRecognizer;
use scanner_types::{
    error::{Result, ScannerError},
    identification::{CardIdentification, IdentificationRequest, IdentificationState},
    images::OriginalImage,
    recognition::RecognitionEvidence,
    scan::{ScanRecord, ScanRequest},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
#[derive(Default)]
struct Recognizer {
    calls: AtomicUsize,
    fail: AtomicBool,
}
#[async_trait::async_trait]
impl CardRecognizer for Recognizer {
    async fn recognize(&self, _: &ScanRecord, _: &OriginalImage) -> Result<RecognitionEvidence> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            return Err(ScannerError::new(
                "catalogue",
                "Fixture catalogue is offline",
            ));
        }
        let mut evidence = RecognitionEvidence::default();
        evidence.extensions.insert(
            "uncataloguedCard".into(),
            serde_json::json!({"name":"Uncatalogued fixture"}),
        );
        Ok(evidence)
    }
}
async fn finished(h: &Harness, id: uuid::Uuid) -> CardIdentification {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(result) = h.engine.card_identification(id, 1).await.unwrap()
                && result.state != IdentificationState::Processing
            {
                return result;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}
async fn awaiting() -> (Harness, Arc<Recognizer>, uuid::Uuid) {
    let recognizer = Arc::new(Recognizer::default());
    let mut h = Harness::with_recognizer(Some(recognizer.clone()));
    let job = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    h.next().await.finish.send(true).unwrap();
    h.terminal(job.id).await;
    while h.engine.status().await.active_job_id.is_some() {
        tokio::task::yield_now().await;
    }
    h.engine
        .identify_card(IdentificationRequest {
            id: uuid::Uuid::new_v4(),
            job_id: job.id,
            sequence: 1,
            language: Default::default(),
            extensions: Default::default(),
        })
        .await
        .unwrap();
    let current = finished(&h, job.id).await;
    // The same-ID publication retry drains the previous owned writer.
    h.engine
        .identify_card(current.request.clone())
        .await
        .unwrap();
    assert_eq!(current.state, IdentificationState::AwaitingCatalogue);
    assert!(current.catalogue_retry_after.unwrap() > Utc::now() + chrono::Duration::hours(23));
    (h, recognizer, job.id)
}
fn make_due(h: &Harness, id: uuid::Uuid) -> CardIdentification {
    let mut record = h.engine.scan_status(id).unwrap();
    let mut current = record.identifications().unwrap().remove(0);
    current.catalogue_retry_after = Some(Utc::now() - chrono::Duration::minutes(1));
    record.replace_identification(&current).unwrap();
    h.engine.database().save(&record).unwrap();
    h.engine.storage().save_manifest(&record).unwrap();
    current
}
#[tokio::test]
async fn due_retries_are_unique_preserve_source_and_back_off_after_catalogue_failure() {
    let (h, recognizer, id) = awaiting().await;
    assert!(
        h.engine
            .retry_awaiting_catalogue(Utc::now())
            .await
            .unwrap()
            .is_none()
    );
    let old = make_due(&h, id);
    recognizer.fail.store(true, Ordering::SeqCst);
    let (first, second) = tokio::join!(
        h.engine.retry_awaiting_catalogue(Utc::now()),
        h.engine.retry_awaiting_catalogue(Utc::now())
    );
    assert_eq!(
        usize::from(first.unwrap().is_some()) + usize::from(second.unwrap().is_some()),
        1
    );
    let failed = finished(&h, id).await;
    assert_eq!(failed.state, IdentificationState::Failed);
    assert_ne!(failed.request.id, old.request.id);
    assert_eq!(failed.source_sha256, old.source_sha256);
    assert!(failed.catalogue_retry_after.unwrap() > Utc::now() + chrono::Duration::hours(23));
    assert!(
        h.engine
            .retry_awaiting_catalogue(Utc::now())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(recognizer.calls.load(Ordering::SeqCst), 2);
    h.engine.shutdown().await;
}
#[tokio::test]
async fn due_cards_wait_while_paused_or_capturing_and_reviewed_cards_are_left_alone() {
    let (mut h, recognizer, id) = awaiting().await;
    make_due(&h, id);
    h.engine.pause().await.unwrap();
    assert!(
        h.engine
            .retry_awaiting_catalogue(Utc::now())
            .await
            .unwrap()
            .is_none()
    );
    h.engine.resume().await.unwrap();
    let active = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    let capture = h.next().await;
    assert!(
        h.engine
            .retry_awaiting_catalogue(Utc::now())
            .await
            .unwrap()
            .is_none()
    );
    capture.finish.send(true).unwrap();
    h.terminal(active.id).await;
    let mut record = h.engine.scan_status(id).unwrap();
    let mut current = record.identifications().unwrap().remove(0);
    current.state = IdentificationState::Rejected;
    record.replace_identification(&current).unwrap();
    h.engine.database().save(&record).unwrap();
    assert!(
        h.engine
            .retry_awaiting_catalogue(Utc::now())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(recognizer.calls.load(Ordering::SeqCst), 1);
    h.engine.shutdown().await;
}
