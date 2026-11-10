//! Local printed cues, distinct from a confirmed catalogue printing.
use crate::images::ImageCrop;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UncataloguedCard {
    pub name: String,
    pub set_code: String,
    pub collector_number: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnergySymbol {
    pub method: String,
    #[serde(rename = "type")]
    pub energy_type: String,
    /// Ink overlap, never a calibrated identity probability.
    pub similarity: f64,
    /// Normalized top-left bounds in the upright card.
    pub region: ImageCrop,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaritySymbol {
    pub method: String,
    pub symbol: String,
    pub similarity: f64,
    pub region: ImageCrop,
    pub collector_number: String,
    pub set_total: String,
}
