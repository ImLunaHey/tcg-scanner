//! Conservative printed set/number/name matching for Star Wars Unlimited.
pub mod printed;
use crate::matching::text::normalized;
use scanner_catalogue::star_wars::{Card, Set};
use scanner_types::recognition::{CardCandidate, RecognizedLine};
pub fn match_card(
    lines: &[RecognizedLine],
    id: &printed::Identifier,
    set: &Set,
    cards: &[Card],
) -> Vec<CardCandidate> {
    if id.set != set.set_id || id.total.is_some_and(|t| t != set.number_cards) {
        return vec![];
    }
    let words: Vec<_> = lines
        .iter()
        .filter(|l| l.confidence >= 0.5)
        .map(|l| normalized(&l.text))
        .collect();
    let matches: Vec<_> = cards
        .iter()
        .filter(|c| {
            c.set == id.set
                && c.number.parse::<u32>().ok() == Some(id.number)
                && words.iter().any(|w| {
                    *w == normalized(&c.name)
                        || (c
                            .subtitle
                            .as_ref()
                            .is_some_and(|s| words.contains(&normalized(s)))
                            && one_substitution(w, &normalized(&c.name)))
                })
        })
        .collect();
    let [card] = matches.as_slice() else {
        return vec![];
    };
    let key = format!("{}-{}", card.set, card.number);
    let mut evidence = vec![
        if words.contains(&normalized(&card.name)) {
            "Exact printed name".into()
        } else {
            "Name with one OCR substitution and exact subtitle".into()
        },
        format!("Printed set code {}", id.set),
        format!("Collector number {}", card.number),
    ];
    if let Some(total) = id.total {
        evidence.push(format!("Printed set total {total}"));
    }
    let mut extensions = scanner_types::Extensions::new();
    extensions.insert("provider".into(), serde_json::json!("swu-db"));
    extensions.insert("catalogueID".into(), serde_json::json!(key));
    if let Some(rarity) = &card.rarity {
        extensions.insert("rarities".into(), serde_json::json!([rarity]));
    }
    vec![CardCandidate {
        id: format!("swu-db:{key}"),
        name: format!(
            "{}{}",
            card.name,
            card.subtitle
                .as_ref()
                .map(|s| format!(" — {s}"))
                .unwrap_or_default()
        ),
        local_id: card.number.clone(),
        set_id: card.set.clone(),
        set_name: set.full_name.clone(),
        official_set_count: Some(set.number_cards),
        evidence,
        category: Some(card.category.clone()),
        types: None,
        available_finishes: vec![],
        extensions,
    }]
}
fn one_substitution(a: &str, b: &str) -> bool {
    a.chars().count() >= 6
        && a.chars().count() == b.chars().count()
        && a.chars().zip(b.chars()).filter(|(a, b)| a != b).count() == 1
}
#[cfg(test)]
mod tests {
    use super::*;
    fn line(text: &str) -> RecognizedLine {
        RecognizedLine {
            text: text.into(),
            confidence: 0.9,
            x: 0.,
            y: 0.8,
            width: 0.8,
            height: 0.1,
        }
    }
    #[test]
    fn matches_require_consistent_names_totals_and_unique_printings() {
        let id = printed::identifiers(["ASH EN 206/264"])
            .into_iter()
            .next()
            .unwrap();
        let mut set = Set {
            set_id: "ASH".into(),
            full_name: "Ashes of the Empire".into(),
            number_cards: 264,
            release_date: None,
        };
        let card = Card {
            set: "ASH".into(),
            number: "206".into(),
            name: "Kelleran Beq".into(),
            subtitle: Some("The Sabered Hand".into()),
            category: "Unit".into(),
            rarity: Some("Rare".into()),
            variant_type: None,
            front_art: None,
        };
        let matched = match_card(
            &[line("Kelleran Beq")],
            &id,
            &set,
            std::slice::from_ref(&card),
        );
        assert_eq!(matched[0].id, "swu-db:ASH-206");
        assert!(matched[0].available_finishes.is_empty());
        assert!(
            match_card(
                &[line("Other Card")],
                &id,
                &set,
                std::slice::from_ref(&card)
            )
            .is_empty()
        );
        assert!(
            match_card(
                &[line("Kelleran Beq")],
                &id,
                &set,
                &[card.clone(), card.clone()]
            )
            .is_empty()
        );
        assert!(
            match_card(
                &[line("Kelleran Beg")],
                &id,
                &set,
                std::slice::from_ref(&card)
            )
            .is_empty()
        );
        assert_eq!(
            match_card(
                &[line("Kelleran Beg"), line("The Sabered Hand")],
                &id,
                &set,
                std::slice::from_ref(&card)
            )
            .len(),
            1
        );
        set.number_cards = 880;
        assert!(match_card(&[line("Kelleran Beq")], &id, &set, &[card]).is_empty());
    }
}
