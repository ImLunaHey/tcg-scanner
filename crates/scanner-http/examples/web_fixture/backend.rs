use scanner_core::backend::{CaptureEvent, CaptureOutcome, CardProcessor, ScannerBackend};
use scanner_types::{
    device::{Device, Inspection},
    error::{Result, ScannerError},
    images::ProcessedImage,
    scan::{ScanRecord, ScanRequest},
};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
pub struct LibraryOnly;
#[async_trait::async_trait]
impl ScannerBackend for LibraryOnly {
    async fn devices(&self) -> Result<Vec<Device>> {
        Ok(vec![])
    }
    async fn inspect(&self, _: &str) -> Result<Inspection> {
        Err(unavailable())
    }
    async fn capture(
        &self,
        _: &str,
        _: &ScanRequest,
        _: PathBuf,
        _: mpsc::Sender<CaptureEvent>,
        _: CancellationToken,
    ) -> Result<CaptureOutcome> {
        Err(unavailable())
    }
}
#[async_trait::async_trait]
impl CardProcessor for LibraryOnly {
    async fn prepare(&self, _: &ScanRecord, _: u32) -> Result<ProcessedImage> {
        Err(unavailable())
    }
}
fn unavailable() -> ScannerError {
    ScannerError::new(
        "unsupported",
        "Browser fixture has no hardware or recognition backend.",
    )
}

#[async_trait::async_trait]
impl scanner_core::backend::CardRecognizer for LibraryOnly {
    async fn recognize(
        &self,
        _: &ScanRecord,
        _: &scanner_types::images::OriginalImage,
    ) -> Result<scanner_types::recognition::RecognitionEvidence> {
        Err(unavailable())
    }
    async fn recognize_camera(
        &self,
        bytes: Vec<u8>,
        _: scanner_types::cards::CardLanguage,
    ) -> Result<scanner_types::camera::CameraIdentification> {
        use scanner_types::{
            camera::{CameraIdentification, CameraState},
            cards::CardGame,
            recognition::{CardCandidate, RecognitionEvidence},
        };
        let card = tokio::task::spawn_blocking(move || scanner_images::camera::extract(&bytes))
            .await
            .map_err(|e| ScannerError::new("fixture", e.to_string()))??;
        if card.is_none() {
            return Ok(CameraIdentification::empty(CameraState::NoCard));
        }
        Ok(CameraIdentification::from_evidence(
            RecognitionEvidence {
                candidates: vec![CardCandidate {
                    id: "synthetic-camera".into(),
                    name: "Synthetic camera card".into(),
                    local_id: "001".into(),
                    set_id: "fixture".into(),
                    set_name: "Browser fixture".into(),
                    official_set_count: Some(1),
                    evidence: vec![
                        "Synthetic browser lifecycle fixture; no OCR identity claim.".into(),
                    ],
                    category: None,
                    types: None,
                    available_finishes: vec![],
                    extensions: Default::default(),
                }],
                ..Default::default()
            },
            CardGame::Pokemon,
        ))
    }
}
