use async_trait::async_trait;
pub use scanner_device::{CaptureEvent, CaptureOutcome, ScannerBackend};
use scanner_types::{error::Result, images::ProcessedImage, scan::ScanRecord};

#[async_trait]
pub trait CardProcessor: Send + Sync {
    /// Implementations preserve originals and return a new immutable derived image.
    async fn prepare(&self, record: &ScanRecord, sequence: u32) -> Result<ProcessedImage>;
}
