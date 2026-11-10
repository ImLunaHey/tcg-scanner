use super::{DinoEncoder, EmbeddingIndex, ReferenceEmbedding, VisualIndexDocument, model_error};
use scanner_types::{cards::CardLanguage, error::Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
pub struct ReferenceSource {
    pub id: String,
    pub path: PathBuf,
    pub sha256: String,
}
#[derive(Deserialize)]
struct Manifest {
    references: Vec<ReferenceSource>,
}

/// Explicit local public-artwork manifest, never scans from the user's ledger.
/// Each immutable source is checksum-verified before deriving a descriptor.
pub fn build_index(
    encoder: &mut DinoEncoder,
    language: CardLanguage,
    manifest: &Path,
    mut progress: impl FnMut(usize, usize),
) -> Result<VisualIndexDocument> {
    let bytes = read_bounded(manifest, 16 * 1024 * 1024)?;
    let source: Manifest = serde_json::from_slice(&bytes).map_err(model_error)?;
    if source.references.is_empty() || source.references.len() > 50_000 {
        return Err(model_error("Expected 1–50,000 reference images."));
    }
    let mut ids = std::collections::BTreeSet::new();
    for r in &source.references {
        if !super::valid_id(&r.id) || !ids.insert(&r.id) {
            return Err(model_error("Invalid or duplicate catalogue ID."));
        }
    }
    let mut references = vec![];
    let total = source.references.len();
    for source in &source.references {
        let bytes = read_bounded(&source.path, 8 * 1024 * 1024)?;
        if format!("{:x}", Sha256::digest(&bytes)) != source.sha256 {
            return Err(model_error("Reference image checksum mismatch."));
        }
        let image = scanner_images::decode::decode(&bytes)?.to_rgb8();
        if u64::from(image.width()) * u64::from(image.height()) > 16_000_000 {
            return Err(model_error("Reference image exceeds 16 megapixels."));
        }
        references.push(ReferenceEmbedding {
            id: source.id.clone(),
            sha256: source.sha256.clone(),
            vector: encoder.encode(&image)?,
        });
        progress(references.len(), total);
    }
    let document = VisualIndexDocument {
        schema_version: 1,
        model: encoder.model_key().into(),
        language,
        references,
    };
    // Apply the same invariants used when a service opens this artifact.
    EmbeddingIndex::new(document.model.clone(), document.references.clone())?;
    Ok(document)
}
fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = vec![];
    std::fs::File::open(path)
        .map_err(model_error)?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(model_error)?;
    if bytes.len() as u64 > limit {
        return Err(model_error("Reference input exceeds its byte limit."));
    }
    Ok(bytes)
}
