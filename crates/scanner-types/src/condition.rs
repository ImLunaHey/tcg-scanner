//! Visible scan observations, separate from a manually assigned selling grade.
use crate::images::ImageCrop;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConditionState {
    Checked,
    Unsupported,
    Inconclusive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FindingKind {
    EdgeWhitening,
    CornerWhitening,
    LinearMark,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConditionFinding {
    pub kind: FindingKind,
    pub region: ImageCrop,
    pub explanation: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionCheck {
    pub method: String,
    pub state: ConditionState,
    pub findings: Vec<ConditionFinding>,
    pub limitations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analyzed_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analyzed_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub back_recognition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<Vec<String>>,
}
impl ConditionCheck {
    pub fn new(state: ConditionState, limitations: Vec<String>) -> Self {
        Self {
            method: "blue-back-visible-marks-rust-v1".into(),
            state,
            findings: vec![],
            limitations,
            analyzed_width: None,
            analyzed_height: None,
            back_recognition: None,
            coverage: None,
        }
    }
}
