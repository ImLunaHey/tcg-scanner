use super::super::support::Harness;
use scanner_core::backend::CardRecognizer;
use scanner_types::{
    camera::{CameraIdentification, CameraRequest, CameraState},
    cards::CardLanguage,
    error::Result,
    images::OriginalImage,
    recognition::RecognitionEvidence,
    scan::{ScanRecord, ScanRequest},
};
use std::sync::Arc;
#[derive(Default)]
struct Blocking {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
#[async_trait::async_trait]
impl CardRecognizer for Blocking {
    async fn recognize(&self, _: &ScanRecord, _: &OriginalImage) -> Result<RecognitionEvidence> {
        panic!("Camera must not invoke a saved-card operation")
    }
    async fn recognize_camera(
        &self,
        bytes: Vec<u8>,
        language: CardLanguage,
    ) -> Result<CameraIdentification> {
        assert_eq!(bytes, b"frame");
        assert_eq!(language, CardLanguage::Korean);
        self.entered.notify_one();
        self.release.notified().await;
        Ok(CameraIdentification::empty(CameraState::NoCard))
    }
}
fn request() -> CameraRequest {
    CameraRequest {
        base64: "ZnJhbWU=".into(),
        language: CardLanguage::Korean,
    }
}
#[tokio::test]
async fn disconnected_camera_keeps_ownership_until_shutdown_can_drain_without_saved_records() {
    let recognizer = Arc::new(Blocking::default());
    let h = Harness::with_recognizer(Some(recognizer.clone()));
    let before = std::fs::read_dir(h.engine.storage().path())
        .unwrap()
        .count();
    let engine = h.engine.clone();
    let caller = tokio::spawn(async move { engine.identify_camera(request()).await });
    recognizer.entered.notified().await;
    caller.abort();
    let _ = caller.await;
    assert_eq!(
        h.engine.identify_camera(request()).await.unwrap_err().kind,
        "busy"
    );
    assert_eq!(h.engine.inspect(None).await.unwrap_err().kind, "busy");
    let engine = h.engine.clone();
    let shutdown = tokio::spawn(async move { engine.shutdown().await });
    while !h.engine.status().await.stopping {
        tokio::task::yield_now().await;
    }
    assert!(!shutdown.is_finished());
    recognizer.release.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(3), shutdown)
        .await
        .unwrap()
        .unwrap();
    assert!(h.engine.jobs(100, 0).await.unwrap().jobs.is_empty());
    assert!(h.engine.events(0, 100, 0).await.unwrap().events.is_empty());
    assert_eq!(
        std::fs::read_dir(h.engine.storage().path())
            .unwrap()
            .count(),
        before
    );
    assert!(h.calls.is_empty());
    assert!(h.inspections.is_empty());
}
#[tokio::test]
async fn queued_capture_waits_for_camera_then_runs_normally() {
    let recognizer = Arc::new(Blocking::default());
    let mut h = Harness::with_recognizer(Some(recognizer.clone()));
    let engine = h.engine.clone();
    let camera = tokio::spawn(async move { engine.identify_camera(request()).await });
    recognizer.entered.notified().await;
    let job = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    assert_eq!(h.engine.status().await.queued_jobs, 1);
    assert!(h.engine.status().await.active_job_id.is_none());
    assert!(h.calls.try_recv().is_err());
    recognizer.release.notify_one();
    assert_eq!(camera.await.unwrap().unwrap().state, CameraState::NoCard);
    let capture = h.next().await;
    assert_eq!(
        h.engine.identify_camera(request()).await.unwrap_err().kind,
        "busy"
    );
    capture.finish.send(true).unwrap();
    h.terminal(job.id).await;
    h.engine.shutdown().await;
}

#[tokio::test]
async fn offline_storage_keeps_camera_and_metadata_available_without_local_fallback() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("offline");
    let root =
        scanner_files::StorageRoot::deferred(scanner_files::configuration::StorageConfiguration {
            storage_root: path.to_string_lossy().into(),
            required_smb_share: None,
        })
        .unwrap();
    let recognizer = Arc::new(Blocking::default());
    let mut h = Harness::with_storage(Some(recognizer.clone()), root);
    assert!(!h.engine.storage_status().await.unwrap().available);
    assert!(h.engine.start_scan(ScanRequest::default()).await.is_err());
    assert!(h.calls.try_recv().is_err());
    let engine = h.engine.clone();
    let camera = tokio::spawn(async move { engine.identify_camera(request()).await });
    recognizer.entered.notified().await;
    recognizer.release.notify_one();
    assert_eq!(camera.await.unwrap().unwrap().state, CameraState::NoCard);
    assert!(h.engine.jobs(50, 0).await.unwrap().jobs.is_empty());
    assert!(!path.exists());
    std::fs::create_dir(&path).unwrap();
    assert!(h.engine.reconnect_storage().await.unwrap().available);
    let job = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    let call = h.next().await;
    assert_eq!(h.engine.reconnect_storage().await.unwrap_err().kind, "busy");
    call.finish.send(true).unwrap();
    assert!(h.terminal(job.id).await.session_closed);
    h.engine.shutdown().await;
}
