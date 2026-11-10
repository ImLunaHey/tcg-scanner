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
