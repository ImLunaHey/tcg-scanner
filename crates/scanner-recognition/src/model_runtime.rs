//! Explicit shared ownership of the process-wide ONNX environment.
use ort::environment::Environment;
use scanner_types::error::{Result, ScannerError};
use std::path::Path;

#[derive(Clone)]
pub struct ModelRuntime {
    pub(crate) environment: Environment,
}
impl ModelRuntime {
    /// Initialize once per process and pass this handle to every model session.
    pub fn load(library: &Path) -> Result<Self> {
        let environment = ort::init_from(library)
            .map_err(error)?
            .with_name("scanner-recognition")
            .with_telemetry(false)
            .build()
            .map_err(error)?;
        Ok(Self { environment })
    }
}

fn error(e: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("recognitionModel", e.to_string())
}
