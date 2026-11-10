use crate::{
    cards::{CardGame, CardLanguage},
    images::OriginalImage,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryCopy {
    #[serde(rename = "batchID")]
    pub batch_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_number: Option<u64>,
    pub card_number: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryPreview {
    #[serde(rename = "batchID")]
    pub batch_id: Uuid,
    pub image: OriginalImage,
    #[serde(default, rename = "editID", skip_serializing_if = "Option::is_none")]
    pub edit_id: Option<Uuid>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorySet {
    pub id: String,
    pub name: String,
    pub game: CardGame,
    pub quantity: usize,
    pub entries: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    pub previews: Vec<InventoryPreview>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryEntry {
    pub id: String,
    pub game: CardGame,
    pub language: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    pub status: String,
    pub quantity: usize,
    pub confirmed_quantity: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<InventoryPreview>,
    pub set_key: String,
    pub copies: Vec<InventoryCopy>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryPage {
    pub schema_version: u32,
    pub sets: Vec<InventorySet>,
    pub release_dates_pending: bool,
    pub entries: Vec<InventoryEntry>,
    pub total_copies: usize,
    pub catalogue_copies: usize,
    pub awaiting_catalogue_copies: usize,
    pub unresolved_copies: usize,
    pub excluded_code_cards: usize,
    pub matching_entries: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryCatalogueSource {
    pub provider: String,
    #[serde(rename = "setID")]
    pub set_id: String,
    pub language: CardLanguage,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryCatalogueCard {
    pub id: String,
    pub name: String,
    pub number: String,
    pub language: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_key: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryMissingCards {
    pub schema_version: u32,
    pub cards: Vec<InventoryCatalogueCard>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalogue_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
