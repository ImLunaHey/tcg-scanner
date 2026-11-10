//! Bounded market catalogue evidence captured 2026-11-01, ported verbatim.
//! Provenance: Docs/stamped-editions.md. Years are candidates, not observations.
use scanner_types::{
    edition::PrintingVariant,
    recognition::{CardCandidate, CardFinish},
};
use serde::Deserialize;
use std::sync::LazyLock;
#[derive(Deserialize)]
struct Entry {
    id: String,
    name: String,
    number: u32,
    total: u32,
    year: u32,
    finishes: Vec<CardFinish>,
    product: u32,
}
static ENTRIES: LazyLock<Vec<Entry>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("trick-or-trade.json")).expect("bundled registry")
});
pub(super) fn matches(card: &CardCandidate) -> Vec<PrintingVariant> {
    let Some((number, total)) = card
        .local_id
        .parse::<u32>()
        .ok()
        .zip(card.official_set_count)
    else {
        return vec![];
    };
    ENTRIES
        .iter()
        .filter(|e| {
            e.id == card.id && e.name == card.name && e.number == number && e.total == total
        })
        .flat_map(|e| {
            e.finishes.iter().map(|&finish| PrintingVariant {
                id: format!(
                    "{}-{}",
                    e.product,
                    serde_json::to_value(finish).unwrap().as_str().unwrap()
                ),
                source: "tcgplayer".into(),
                finish,
                stamps: vec!["trick-or-trade".into()],
                years: vec![e.year],
            })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_requires_name_number_total_and_exact_catalogue_identity() {
        let mut card:CardCandidate=serde_json::from_value(serde_json::json!({"id":"sv01-104","name":"Greavard",
            "localID":"104","setID":"sv01","setName":"Scarlet & Violet","officialSetCount":198,"evidence":[]})).unwrap();
        assert_eq!(ENTRIES.len(), 90);
        let variants = matches(&card);
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].id, "515677-normal");
        card.name = "Other".into();
        assert!(matches(&card).is_empty());
        card.name = "Greavard".into();
        card.official_set_count = Some(197);
        assert!(matches(&card).is_empty());
        card.official_set_count = Some(198);
        card.id = "sv01-0104".into();
        assert!(matches(&card).is_empty());
    }
}
