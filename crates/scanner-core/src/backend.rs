use async_trait::async_trait;
pub use scanner_device::{CaptureEvent, CaptureOutcome, ScannerBackend};
use scanner_types::{error::Result, images::ProcessedImage, scan::ScanRecord};

#[async_trait]
pub trait CardProcessor: Send + Sync {
    /// Implementations preserve originals and return a new immutable derived image.
    async fn prepare(&self, record: &ScanRecord, sequence: u32) -> Result<ProcessedImage>;
}

#[async_trait]
pub trait CardRecognizer: Send + Sync {
    /// The record and source are verified snapshots; implementations return
    /// suggestions only, never manual confirmation or a condition grade.
    async fn recognize(
        &self,
        record: &ScanRecord,
        image: &scanner_types::images::OriginalImage,
    ) -> Result<scanner_types::recognition::RecognitionEvidence>;
}
