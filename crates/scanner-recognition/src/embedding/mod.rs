//! Visual retrieval returns candidates, never automatic identity confirmation.
mod index;
mod onnx;

pub use index::{EmbeddingIndex, ReferenceEmbedding, VisualCandidate};
pub use onnx::{DinoEncoder, MODEL_REVISION};

use scanner_types::error::{Result, ScannerError};

fn model_error(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("recognitionModel", error.to_string())
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
