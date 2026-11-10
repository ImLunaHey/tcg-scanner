use scanner_types::{printed::RaritySymbol, recognition::CardCandidate};
fn expected(rarity: &str) -> Option<&'static str> {
    match rarity.to_lowercase().as_str() {
        "common" => Some("circle"),
        "uncommon" => Some("diamond"),
        "rare" | "rare holo" | "holo rare" => Some("star"),
        "double rare" => Some("doubleStar"),
        _ => None,
    }
}
/// Corroborate only the same printed number and total. Keep every candidate,
/// including conflicts, and leave finish/price/confirmation untouched.
pub fn apply(reading: &RaritySymbol, candidates: &mut [CardCandidate]) {
    let mut ranks = std::collections::BTreeMap::new();
    for card in candidates.iter_mut() {
        let mut rank = 0;
        if crate::matching::text::number(&card.local_id)
            == crate::matching::text::number(&reading.collector_number)
            && card
                .official_set_count
                .is_some_and(|n| Some(n) == reading.set_total.parse().ok())
            && let Some(rarity) = card
                .extensions
                .get("rarities")
                .and_then(|v| v.as_array())
                .and_then(|v| v.first())
                .and_then(|v| v.as_str())
            && let Some(expected) = expected(rarity)
        {
            if reading.symbol == expected {
                card.evidence.push(format!(
                    "Printed black rarity symbol agrees with catalogue rarity: {rarity}"
                ));
                rank = 1;
            } else {
                card.evidence.push(format!(
                    "Rarity symbol conflict: printed {}, catalogue {rarity}; review identity",
                    reading.symbol
                ));
                rank = -1;
            }
        }
        ranks.insert(card.id.clone(), rank);
    }
    candidates.sort_by_key(|c| std::cmp::Reverse(ranks[&c.id]));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn card(id: &str, rarity: &str, number: &str) -> CardCandidate {
        serde_json::from_value(serde_json::json!({"id":id,"name":"Card","localID":number,"setID":"s","setName":"Set","officialSetCount":100,"evidence":[],"rarities":[rarity],"availableFinishes":["normal","holo"]})).unwrap()
    }
    #[test]
    fn corroboration_never_hides_conflicts_changes_finish_or_bypasses_numbers() {
        let reading = RaritySymbol {
            method: "test".into(),
            symbol: "circle".into(),
            similarity: 0.9,
            region: Default::default(),
            collector_number: "01".into(),
            set_total: "100".into(),
        };
        let mut cards = vec![
            card("conflict", "Rare", "1"),
            card("different", "Common", "2"),
            card("agrees", "Common", "001"),
        ];
        let finishes = cards[0].available_finishes.clone();
        apply(&reading, &mut cards);
        assert_eq!(
            cards.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            ["agrees", "different", "conflict"]
        );
        assert!(cards[0].evidence[0].contains("agrees"));
        assert!(cards[1].evidence.is_empty());
        assert!(cards[2].evidence[0].starts_with("Rarity symbol conflict:"));
        assert!(cards.iter().all(|c| c.available_finishes == finishes));
    }
}
