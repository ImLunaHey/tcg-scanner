mod native;
mod tsv;

pub use native::Tesseract;
pub use tsv::RecognizedLine;

use scanner_types::error::ScannerError;

fn ocr_error(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("recognition", error.to_string())
}
