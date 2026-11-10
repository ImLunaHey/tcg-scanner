use crate::Extensions;
use serde::{Deserialize, Serialize};
/// Normalized bottom-left coordinates in the orientation used for OCR.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecognizedLine {
    pub text: String,
    pub confidence: f32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardFinish {
    #[default]
    Unverified,
    Normal,
    Holo,
    ReverseHolo,
    Other,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardCondition {
    #[default]
    Unassessed,
    NearMint,
    LightlyPlayed,
    ModeratelyPlayed,
    HeavilyPlayed,
    Damaged,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardCandidate {
    pub id: String,
    pub name: String,
    #[serde(rename = "localID")]
    pub local_id: String,
    #[serde(rename = "setID")]
    pub set_id: String,
    pub set_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub official_set_count: Option<u32>,
    pub evidence: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<String>>,
    #[serde(default)]
    pub available_finishes: Vec<CardFinish>,
    #[serde(flatten)]
    pub extensions: Extensions,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionEvidence {
    pub lines: Vec<RecognizedLine>,
    pub candidates: Vec<CardCandidate>,
    pub warnings: Vec<String>,
    pub orientation: u32,
    #[serde(flatten)]
    pub extensions: Extensions,
}
