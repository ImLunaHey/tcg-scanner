//! Similar artwork is an optional review aid, separate from printed identity.
use crate::recognition::{CardCandidate, RecognitionEvidence};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualMatch {
    pub candidate: CardCandidate,
    pub similarity: f32,
}
impl RecognitionEvidence {
    pub fn visual_matches(&self) -> Vec<VisualMatch> {
        let Some(values) = self
            .extensions
            .get("visualMatches")
            .and_then(|v| v.as_array())
        else {
            return vec![];
        };
        if values.len() > 5 {
            return vec![];
        }
        values
            .iter()
            .filter_map(|v| serde_json::from_value::<VisualMatch>(v.clone()).ok())
            .filter(|v| {
                v.similarity.is_finite()
                    && (-1.0..=1.0).contains(&v.similarity)
                    && !v.candidate.id.is_empty()
                    && v.candidate.id.len() <= 100
            })
            .collect()
    }
    /// Only explicit human review may promote a visual match to a printing.
    /// Automatic price/finish/Store paths continue to read `candidates` only.
    pub fn review_candidates(&self) -> Vec<CardCandidate> {
        let mut result = self.candidates.clone();
        for visual in self.visual_matches() {
            if !result.iter().any(|c| c.id == visual.candidate.id) {
                result.push(visual.candidate);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visual_matches_do_not_become_printed_candidates() {
        let mut evidence = RecognitionEvidence::default();
        evidence.extensions.insert("visualMatches".into(),serde_json::json!([{"similarity":0.93,"candidate":{"id":"sv1-1","name":"Card","localID":"1","setID":"sv1","setName":"Set","evidence":["Similar artwork; verify printing"]}}]));
        assert_eq!(evidence.visual_matches().len(), 1);
        assert_eq!(evidence.review_candidates().len(), 1);
        assert!(evidence.candidates.is_empty());
        evidence.extensions.get_mut("visualMatches").unwrap()[0]["similarity"] =
            serde_json::json!(1.5);
        assert!(evidence.review_candidates().is_empty());
    }
}
