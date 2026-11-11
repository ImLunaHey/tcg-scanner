use super::{DinoEncoder, EmbeddingIndex, ReferenceEmbedding, VisualCandidate, model_error};
use scanner_types::{cards::CardLanguage, error::Result};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::sync::Semaphore;

/// Persisted public-artwork descriptors. Version includes preprocessing and CLS
/// selection, so incompatible model/index pairs never silently compare vectors.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisualIndexDocument {
    pub schema_version: u32,
    pub model: String,
    pub language: CardLanguage,
    pub references: Vec<ReferenceEmbedding>,
}
impl VisualIndexDocument {
    pub fn load(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path).map_err(model_error)?;
        if file.metadata().map_err(model_error)?.len() > 256 * 1024 * 1024 {
            return Err(model_error("Visual index exceeds 256 MiB."));
        }
        let mut bytes = vec![];
        file.take(256 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(model_error)?;
        if bytes.len() > 256 * 1024 * 1024 {
            return Err(model_error("Visual index exceeds 256 MiB."));
        }
        serde_json::from_slice(&bytes).map_err(model_error)
    }
}

pub struct VisualRetriever {
    encoder: Arc<Mutex<DinoEncoder>>,
    index: Arc<EmbeddingIndex>,
    worker: Arc<Semaphore>,
    model: String,
    language: CardLanguage,
    references: usize,
}
impl VisualRetriever {
    pub fn load(runtime: &Path, model: &Path, index: &Path) -> Result<Self> {
        let runtime = crate::model_runtime::ModelRuntime::load(runtime)?;
        Self::with_runtime(&runtime, model, index)
    }
    pub fn with_runtime(
        runtime: &crate::model_runtime::ModelRuntime,
        model: &Path,
        index: &Path,
    ) -> Result<Self> {
        let document = VisualIndexDocument::load(index)?;
        let encoder = DinoEncoder::with_runtime(runtime, model)?;
        if document.schema_version != 1
            || document.model != encoder.model_key()
            || document.references.iter().any(|r| r.vector.len() != 384)
        {
            return Err(model_error(
                "Visual index does not match the pinned model/preprocessing.",
            ));
        }
        let references = document.references.len();
        let index = EmbeddingIndex::new(document.model.clone(), document.references)?;
        Ok(Self {
            encoder: Arc::new(Mutex::new(encoder)),
            index: Arc::new(index),
            worker: Arc::new(Semaphore::new(1)),
            model: document.model,
            language: document.language,
            references,
        })
    }
    pub fn model(&self) -> &str {
        &self.model
    }
    pub fn reference_count(&self) -> usize {
        self.references
    }
    /// One owned native inference; a disconnected client cannot free its permit
    /// while ONNX is still running. English references never substitute for Korean.
    pub async fn search(
        &self,
        bytes: Vec<u8>,
        turns: u32,
        language: CardLanguage,
    ) -> Result<Vec<VisualCandidate>> {
        if language != self.language {
            return Ok(vec![]);
        }
        if turns > 3 {
            return Err(model_error("Invalid visual orientation."));
        }
        let permit = self
            .worker
            .clone()
            .try_acquire_owned()
            .map_err(|_| model_error("Visual recognition is busy."))?;
        let (encoder, index, model) =
            (self.encoder.clone(), self.index.clone(), self.model.clone());
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let pixels = scanner_images::decode::decode(&bytes)?.to_rgb8();
            let rotated = match turns {
                1 => image::imageops::rotate90(&pixels),
                2 => image::imageops::rotate180(&pixels),
                3 => image::imageops::rotate270(&pixels),
                _ => pixels,
            };
            let vector = encoder
                .lock()
                .map_err(|_| model_error("Visual encoder failed."))?
                .encode(&rotated)?;
            index.search(&model, &vector, 5)
        })
        .await
        .map_err(model_error)?
    }
}
