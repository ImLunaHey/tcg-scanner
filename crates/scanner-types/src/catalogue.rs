use crate::Extensions;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueCard {
    pub id: String,
    pub local_id: String,
    pub name: String,
    pub image: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CardCounts {
    pub official: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSet {
    pub id: String,
    pub name: String,
    pub card_count: Option<CardCounts>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailedVariant {
    pub variant_id: Option<String>,
    #[serde(rename = "type")]
    pub variant_type: String,
    pub size: Option<String>,
    pub stamp: Option<Vec<String>>,
    pub subtype: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardDetail {
    pub id: String,
    pub local_id: String,
    pub name: String,
    pub set: CardSet,
    pub image: Option<String>,
    pub variants: Option<std::collections::BTreeMap<String, bool>>,
    pub rarity: Option<String>,
    pub category: Option<String>,
    pub types: Option<Vec<String>>,
    #[serde(rename = "variants_detailed")]
    pub variants_detailed: Option<Vec<DetailedVariant>>,
    #[serde(flatten)]
    pub extensions: Extensions,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDetail {
    pub id: String,
    pub release_date: Option<String>,
    pub cards: Vec<CatalogueCard>,
}
