//! Visual edition evidence, independent of manually confirmed finish or grade.
use crate::{images::ImageCrop, recognition::CardFinish};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StampState {
    Present,
    Absent,
    Uncertain,
    Unsupported,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StampEvidence {
    pub state: StampState,
    pub kind: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<ImageCrop>,
    pub source_quarter_turns: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub similarity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_similarity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downsampled_similarity: Option<f64>,
    pub evidence: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintingVariant {
    pub id: String,
    pub source: String,
    pub finish: CardFinish,
    pub stamps: Vec<String>,
    #[serde(default)]
    pub years: Vec<u32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditionState {
    NotDetected,
    Matched,
    Uncertain,
    Unlisted,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditionEvidence {
    pub state: EditionState,
    #[serde(
        default,
        rename = "candidateID",
        skip_serializing_if = "Option::is_none"
    )]
    pub candidate_id: Option<String>,
    pub stamp: StampEvidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub variants: Vec<PrintingVariant>,
    pub evidence: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<ReferenceEvidence>,
    pub pricing_key: String,
}
impl EditionEvidence {
    pub fn new(
        state: EditionState,
        candidate_id: Option<String>,
        stamp: StampEvidence,
        name: Option<String>,
        variants: Vec<PrintingVariant>,
        evidence: Vec<String>,
    ) -> Self {
        let raw = |value| serde_json::from_value::<String>(value).expect("wire enum string");
        let mut key = vec![
            "edition-v1".to_owned(),
            candidate_id.clone().unwrap_or("unknown".into()),
            raw(serde_json::to_value(state).unwrap()),
            stamp.kind.clone(),
            raw(serde_json::to_value(stamp.state).unwrap()),
        ];
        let mut entries: Vec<_> = variants
            .iter()
            .map(|v| {
                let mut stamps = v.stamps.clone();
                stamps.sort();
                let mut years = v.years.clone();
                years.sort();
                format!(
                    "{}:{}:{}:{}:{}",
                    v.source,
                    v.id,
                    raw(serde_json::to_value(v.finish).unwrap()),
                    stamps.join(","),
                    years
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect();
        entries.sort();
        key.extend(entries);
        Self {
            state,
            candidate_id,
            stamp,
            name,
            variants,
            evidence,
            reference: None,
            pricing_key: key.join("|"),
        }
    }
    pub fn needs_edition_handling(&self) -> bool {
        matches!(
            self.stamp.state,
            StampState::Present | StampState::Uncertain
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReferenceState {
    Compared,
    Missing,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceEvidence {
    pub state: ReferenceState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stamp_state: Option<StampState>,
    pub explanation: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pricing_key_is_order_independent_and_invalidates_ordinary_quotes() {
        let stamp = StampEvidence {
            state: StampState::Present,
            kind: "pikachu-pumpkin".into(),
            method: "test".into(),
            region: None,
            source_quarter_turns: 0,
            similarity: None,
            baseline_similarity: None,
            downsampled_similarity: None,
            evidence: vec![],
        };
        let a = PrintingVariant {
            id: "12-normal".into(),
            source: "tcgplayer".into(),
            finish: CardFinish::Normal,
            stamps: vec!["trick-or-trade".into()],
            years: vec![2023, 2022],
        };
        let mut b = a.clone();
        b.id = "13-holo".into();
        b.finish = CardFinish::Holo;
        let first = EditionEvidence::new(
            EditionState::Matched,
            Some("sv01-104".into()),
            stamp.clone(),
            None,
            vec![a.clone(), b.clone()],
            vec![],
        );
        let mut reversed = a.clone();
        reversed.years.reverse();
        let second = EditionEvidence::new(
            EditionState::Matched,
            Some("sv01-104".into()),
            stamp.clone(),
            None,
            vec![b, reversed],
            vec![],
        );
        assert_eq!(first.pricing_key, second.pricing_key);
        assert!(first.needs_edition_handling());
        assert!(
            first
                .pricing_key
                .starts_with("edition-v1|sv01-104|matched|pikachu-pumpkin|present|")
        );
        let ordinary = EditionEvidence::new(
            EditionState::NotDetected,
            Some("sv01-104".into()),
            StampEvidence {
                state: StampState::Absent,
                ..stamp
            },
            None,
            vec![],
            vec![],
        );
        assert!(!ordinary.needs_edition_handling());
        assert_ne!(first.pricing_key, ordinary.pricing_key);
        let restored: EditionEvidence =
            serde_json::from_value(serde_json::to_value(first).unwrap()).unwrap();
        assert_eq!(restored.pricing_key, second.pricing_key);
    }
}
