//! Visual retrieval returns candidates, never automatic identity confirmation.
mod build;
mod index;
pub use build::{ReferenceSource, build_index};
mod onnx;
mod retriever;
pub use retriever::{VisualIndexDocument, VisualRetriever};

pub use index::{EmbeddingIndex, ReferenceEmbedding, VisualCandidate};
pub use onnx::{DinoEncoder, MODEL_REVISION};

use scanner_types::error::{Result, ScannerError};

fn model_error(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("recognitionModel", error.to_string())
}
fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+.-".contains(&b))
}

fn normalize(values: &mut [f32]) -> Result<()> {
    if values.is_empty() || values.iter().any(|v| !v.is_finite()) {
        return Err(model_error("Invalid embedding."));
    }
    let norm = values
        .iter()
        .map(|v| f64::from(*v).powi(2))
        .sum::<f64>()
        .sqrt();
    if norm < 1e-12 {
        return Err(model_error("Zero embedding."));
    }
    for value in values {
        *value = (f64::from(*value) / norm) as f32;
    }
    Ok(())
}
