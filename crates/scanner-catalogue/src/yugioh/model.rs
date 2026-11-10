use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Printing {
    pub set_name: String,
    pub set_code: String,
    pub set_rarity: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Artwork {
    pub id: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Card {
    pub id: u64,
    pub name: String,
    #[serde(rename = "type")]
    pub category: String,
    pub card_sets: Option<Vec<Printing>>,
    pub card_images: Option<Vec<Artwork>>,
}
#[derive(Deserialize)]
pub(super) struct Envelope {
    pub data: Vec<Card>,
}
#[derive(Deserialize)]
pub(super) struct SetDate {
    pub set_code: String,
    pub tcg_date: Option<String>,
}
pub(super) fn valid(cards: &[Card]) -> bool {
    let unique: std::collections::HashSet<_> = cards.iter().map(|c| c.id).collect();
    !cards.is_empty()
        && cards.len() <= 100_000
        && unique.len() == cards.len()
        && cards.iter().all(|c| {
            (1..=9_999_999_999).contains(&c.id)
                && !c.name.is_empty()
                && c.name.len() <= 256
                && c.category.len() <= 128
                && c.card_sets.as_ref().is_none_or(|s| {
                    s.len() <= 1000
                        && s.iter().all(|p| {
                            p.set_code.len() <= 40
                                && p.set_name.len() <= 256
                                && p.set_rarity.len() <= 128
                        })
                })
                && c.card_images.as_ref().is_none_or(|s| {
                    s.len() <= 100 && s.iter().all(|i| (1..=9_999_999_999).contains(&i.id))
                })
        })
}
