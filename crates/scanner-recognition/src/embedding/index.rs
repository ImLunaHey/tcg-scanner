use super::{model_error, normalize};
use scanner_types::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceEmbedding {
    pub id: String,
    pub sha256: String,
    pub vector: Vec<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisualCandidate {
    pub id: String,
    pub similarity: f32,
}

/// Exact cosine search is sufficient for the initial catalogue; backend can be replaced
/// without changing evidence semantics. Never compare different preprocessing versions.
pub struct EmbeddingIndex {
    model: String,
    dimension: usize,
    references: Vec<ReferenceEmbedding>,
}

impl EmbeddingIndex {
    pub fn new(model: String, mut references: Vec<ReferenceEmbedding>) -> Result<Self> {
        let dimension = references.first().map(|r| r.vector.len()).unwrap_or(0);
        if model.is_empty() || dimension == 0 || dimension > 4096 {
            return Err(model_error("Invalid index model or dimensions."));
        }
        for reference in &mut references {
            if reference.id.is_empty()
                || reference.vector.len() != dimension
                || reference.sha256.len() != 64
                || !reference.sha256.bytes().all(|c| c.is_ascii_hexdigit())
            {
                return Err(model_error("Invalid reference embedding."));
            }
            normalize(&mut reference.vector)?;
        }
        Ok(Self {
            model,
            dimension,
            references,
        })
    }
    pub fn search(&self, model: &str, query: &[f32], limit: usize) -> Result<Vec<VisualCandidate>> {
        if model != self.model || query.len() != self.dimension || !(1..=50).contains(&limit) {
            return Err(model_error(
                "Query model, dimensions or result limit do not match index.",
            ));
        }
        let mut query = query.to_vec();
        normalize(&mut query)?;
        let mut candidates: Vec<_> = self
            .references
            .iter()
            .map(|reference| VisualCandidate {
                id: reference.id.clone(),
                similarity: reference
                    .vector
                    .iter()
                    .zip(&query)
                    .map(|(a, b)| a * b)
                    .sum::<f32>()
                    .clamp(-1., 1.),
            })
            .collect();
        candidates.sort_by(|a, b| b.similarity.total_cmp(&a.similarity).then(a.id.cmp(&b.id)));
        candidates.truncate(limit);
        Ok(candidates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranks_by_cosine_and_refuses_mixed_models_or_nonfinite_vectors() {
        let reference = |id: &str, v| ReferenceEmbedding {
            id: id.into(),
            sha256: "a".repeat(64),
            vector: v,
        };
        let index = EmbeddingIndex::new(
            "v1".into(),
            vec![reference("a", vec![4., 0.]), reference("b", vec![0., 2.])],
        )
        .unwrap();
        assert_eq!(index.search("v1", &[0., 7.], 2).unwrap()[0].id, "b");
        assert!(index.search("v2", &[0., 1.], 2).is_err());
        assert!(index.search("v1", &[f32::NAN, 1.], 2).is_err());
        assert!(EmbeddingIndex::new("v1".into(), vec![reference("a", vec![0., 0.])]).is_err());
    }
}
