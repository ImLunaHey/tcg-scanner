use crate::{Extensions, images::ImageCrop, recognition::CardFinish};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FinishState {
    Estimated,
    #[default]
    Uncertain,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FinishUncertainty {
    AmbiguousIdentity,
    MarkerNotFound,
    MarkerContrast,
    ConflictingContrast,
    MultipleRegularFinishes,
    MissingRegularFinish,
    AnalysisFailed,
    ConflictingFinishEvidence,
    StampUncertain,
    EditionUnavailable,
    MultipleEditionFinishes,
    LanguageUnsupported,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishEstimate {
    pub state: FinishState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_finish: Option<CardFinish>,
    #[serde(
        default,
        rename = "candidateID",
        skip_serializing_if = "Option::is_none"
    )]
    pub candidate_id: Option<String>,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_evidence: Option<BorderFinishEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertainty_reason: Option<FinishUncertainty>,
    pub evidence: Vec<String>,
    pub limitations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<ImageCrop>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_quarter_turns: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker_polarity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub median_luminance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dark_background_area_fraction: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light_background_area_fraction: Option<f64>,
    #[serde(flatten)]
    pub extensions: Extensions,
}
impl FinishEstimate {
    pub fn new(
        state: FinishState,
        candidate_id: Option<String>,
        evidence: Vec<String>,
        reason: Option<FinishUncertainty>,
    ) -> Self {
        Self {
            state,
            candidate_id,
            evidence,
            uncertainty_reason: reason,
            method: "modern-set-code-and-border-rust-v1".into(),
            suggested_finish: None,
            border_evidence: None,
            limitations: vec![],
            region: None,
            source_quarter_turns: None,
            marker_polarity: None,
            median_luminance: None,
            dark_background_area_fraction: None,
            light_background_area_fraction: None,
            extensions: Extensions::new(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BorderState {
    Present,
    Absent,
    Unstable,
    Unsupported,
    Failed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BorderFinishEvidence {
    pub state: BorderState,
    pub threshold: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inset: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downsampled: Option<f64>,
    pub method: String,
}
