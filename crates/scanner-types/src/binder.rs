//! Normalized, top-left photo coordinates. Boxes locate cards, not identities.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CardBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub confidence: f32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinderDetection {
    pub schema_version: u32,
    pub method: String,
    pub cards: Vec<CardBox>,
}
