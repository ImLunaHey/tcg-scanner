//! Version-one Store wire contracts. Optional ownership fields serialize as null.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Import {
    pub scanner_instance_id: String,
    pub source_batch_id: Uuid,
    pub batch_number: String,
    pub cards: Vec<CardInput>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardInput {
    pub card_number: String,
    pub review: Review,
    pub game: Option<String>,
    pub language: Option<String>,
    pub game_source: Option<String>,
    pub language_source: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub recognition_suggestions: Vec<Suggestion>,
    pub finish_estimate: Option<String>,
    pub condition_estimate: Option<String>,
    pub border_findings: Vec<String>,
    pub suggested_price_gbp: Option<i64>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub provider: Option<String>,
    pub external_id: Option<String>,
    pub name: String,
    pub set_name: Option<String>,
    pub collector_number: Option<String>,
    pub language: Option<String>,
    pub catalogue_id: Option<String>,
    pub confidence: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchReply {
    pub id: Uuid,
    pub scanner_instance_id: String,
    pub source_batch_id: Uuid,
    pub batch_number: String,
    pub review_url: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Photo {
    pub id: Uuid,
    pub sha256: String,
    pub size_bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardReply {
    pub id: Uuid,
    pub scan_batch_id: Uuid,
    pub card_number: String,
    pub revision: u64,
    pub status: String,
    pub front: Option<Photo>,
    pub back: Option<Photo>,
    pub review: Option<Review>,
    pub confirmed: Option<serde_json::Value>,
    pub notes: Option<String>,
    pub game: Option<String>,
    pub language: Option<String>,
    pub game_source: Option<String>,
    pub language_source: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportReply {
    pub batch: BatchReply,
    pub cards: Vec<CardReply>,
}
