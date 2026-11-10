use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Card {
    pub set: String,
    pub number: String,
    pub name: String,
    pub subtitle: Option<String>,
    #[serde(rename = "Type")]
    pub category: String,
    pub rarity: Option<String>,
    pub variant_type: Option<String>,
    pub front_art: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Set {
    pub set_id: String,
    pub full_name: String,
    pub number_cards: u32,
    pub release_date: Option<String>,
}
#[derive(Deserialize)]
pub(super) struct Envelope {
    pub data: Vec<Card>,
}
pub(super) fn valid_id(id: &str) -> bool {
    (3..=8).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}
pub(super) fn valid_cards(cards: &[Card], set: &str) -> bool {
    let unique: std::collections::HashSet<_> = cards.iter().map(|c| &c.number).collect();
    !cards.is_empty()
        && cards.len() <= 5000
        && unique.len() == cards.len()
        && cards.iter().all(|c| {
            c.set == set
                && !c.number.is_empty()
                && c.number.len() <= 16
                && !c.name.is_empty()
                && c.name.len() <= 160
                && c.subtitle.as_ref().is_none_or(|s| s.len() <= 160)
                && c.category.len() <= 80
                && c.rarity.as_ref().is_none_or(|s| s.len() <= 80)
                && c.variant_type.as_ref().is_none_or(|s| s.len() <= 80)
                && c.front_art.as_ref().is_none_or(|s| s.len() <= 512)
        })
}
