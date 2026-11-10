use super::super::support::Harness;
use scanner_core::backend::CardRecognizer;
use scanner_types::{
    error::Result,
    identification::IdentificationState,
    images::OriginalImage,
    recognition::RecognitionEvidence,
    scan::{ScanRecord, ScanRequest},
};
use std::sync::{Arc, Mutex};
struct Recognizer(Mutex<Vec<u32>>);
#[async_trait::async_trait]
impl CardRecognizer for Recognizer {
    async fn recognize(
        &self,
        _: &ScanRecord,
        image: &OriginalImage,
    ) -> Result<RecognitionEvidence> {
        self.0.lock().unwrap().push(image.sequence);
        Ok(RecognitionEvidence::default())
    }
}
#[tokio::test]
async fn automatic_preparation_and_recognition_publish_in_capture_order() {
    let recognizer = Arc::new(Recognizer(Mutex::new(vec![])));
    let mut h = Harness::with_recognizer(Some(recognizer.clone()));
    let job = h
        .engine
        .start_scan(ScanRequest {
            prepare_cards: Some(true),
            duplex: true,
            ..Default::default()
        })
        .await
        .unwrap();
    h.next().await.finish.send(true).unwrap();
    let record = h.terminal(job.id).await;
    assert_eq!(*recognizer.0.lock().unwrap(), [1, 2]);
    let results = record.identifications().unwrap();
    assert_eq!(results.len(), 2);
    assert!(
        results
            .iter()
            .all(|i| i.state == IdentificationState::NoMatch)
    );
    assert!(results.iter().all(|i| {
        record
            .images
            .iter()
            .any(|image| image.sequence == i.request.sequence && image.sha256 == i.source_sha256)
    }));
    assert_eq!(record.extensions["cardGames"][0]["game"], "unknown");
    h.engine.shutdown().await;
}

struct Blocking {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
#[async_trait::async_trait]
impl CardRecognizer for Blocking {
    async fn recognize(&self, _: &ScanRecord, _: &OriginalImage) -> Result<RecognitionEvidence> {
        self.entered.notify_one();
        self.release.notified().await;
        Ok(RecognitionEvidence::default())
    }
}
#[tokio::test]
async fn shutdown_cancels_hardware_while_draining_an_owned_saved_image_task() {
    let recognizer = Arc::new(Blocking {
        entered: Default::default(),
        release: Default::default(),
    });
    let mut h = Harness::with_recognizer(Some(recognizer.clone()));
    let saved = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    h.next().await.finish.send(true).unwrap();
    h.terminal(saved.id).await;
    while h.engine.status().await.active_job_id.is_some() {
        tokio::task::yield_now().await;
    }
    let active = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    let capture = h.next().await;
    let request = scanner_types::identification::IdentificationRequest {
        id: uuid::Uuid::new_v4(),
        job_id: saved.id,
        sequence: 1,
        language: Default::default(),
        extensions: Default::default(),
    };
    h.engine.identify_card(request).await.unwrap();
    recognizer.entered.notified().await;
    let shutdown = {
        let engine = h.engine.clone();
        tokio::spawn(async move { engine.shutdown().await })
    };
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        capture.cancel.cancelled(),
    )
    .await
    .unwrap();
    assert!(!shutdown.is_finished());
    capture.finish.send(true).unwrap();
    recognizer.release.notify_one();
    shutdown.await.unwrap();
    assert_eq!(
        h.engine.scan_status(active.id).unwrap().state,
        scanner_types::scan::ScanState::Cancelled
    );
    assert_eq!(
        h.engine
            .card_identification(saved.id, 1)
            .await
            .unwrap()
            .unwrap()
            .state,
        IdentificationState::NoMatch
    );
}
