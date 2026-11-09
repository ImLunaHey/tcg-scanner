use async_trait::async_trait;
use scanner_types::{
    Extensions,
    device::{Device, Inspection},
    error::{Result, ScannerError},
    images::ProcessedImage,
    scan::{ScanRecord, ScanRequest},
};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub enum CaptureEvent {
    Settings(Extensions),
    Image { filename: String, sequence: u32 },
}
pub struct CaptureOutcome {
    pub errors: Vec<ScannerError>,
    pub cancelled: bool,
    pub session_closed: bool,
}

#[async_trait]
pub trait ScannerBackend: Send + Sync {
    async fn devices(&self) -> Result<Vec<Device>>;
    async fn inspect(&self, device_id: &str) -> Result<Inspection>;
    /// Cancellation is cooperative: return only after cleanup and report whether closure
    /// was confirmed. The coordinator never drops this future to abandon a session.
    async fn capture(
        &self,
        device_id: &str,
        request: &ScanRequest,
        incoming: PathBuf,
        events: mpsc::Sender<CaptureEvent>,
        cancel: CancellationToken,
    ) -> Result<CaptureOutcome>;
    async fn recover_after_disconnect(&self) -> bool {
        false
    }
    async fn wait_for_scan_button(
        &self,
        _device_id: &str,
        _cancel: CancellationToken,
    ) -> Result<()> {
        Err(ScannerError::new(
            "unsupported",
            "This driver does not expose a scan-button monitor.",
        ))
    }
    async fn observe_button(
        &self,
        _device_id: &str,
        _seconds: u32,
        _cancel: CancellationToken,
    ) -> Result<()> {
        Err(ScannerError::new(
            "unsupported",
            "This driver does not expose button diagnostics.",
        ))
    }
}

#[async_trait]
pub trait CardProcessor: Send + Sync {
    /// Implementations preserve originals and return a new immutable derived image.
    async fn prepare(&self, record: &ScanRecord, sequence: u32) -> Result<ProcessedImage>;
}
