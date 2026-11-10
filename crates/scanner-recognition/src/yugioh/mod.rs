//! Exact English identity and printed-set evidence; never infers physical foil.
mod text;
use scanner_catalogue::yugioh::Card;
use scanner_types::recognition::{CardCandidate, RecognizedLine};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
pub struct Match {
    pub candidates: Vec<CardCandidate>,
    pub score: usize,
    pub warnings: Vec<String>,
}
impl Match {
    fn warning(text: &str) -> Self {
        Self {
            warnings: vec![text.into()],
            ..Default::default()
        }
    }
}
pub fn match_card(lines: &[RecognizedLine], cards: &[Card]) -> Match {
    let read = text::read(lines);
    let by_name: Vec<_> = cards
        .iter()
        .filter(|c| read.titles.contains(&text::normalized(&c.name)))
        .collect();
    let by_code: Vec<_> = cards
        .iter()
        .filter(|c| read.passcodes.contains(&c.id))
        .collect();
    if read.passcodes.len() > 1
        || by_name.len() > 1
        || by_code.len() > 1
        || (!by_name.is_empty() && !by_code.is_empty() && by_name[0].id != by_code[0].id)
    {
        return Match::warning("Conflicting name or passcode evidence; no card was selected.");
    }
    let Some(card) = by_code.first().or_else(|| by_name.first()) else {
        return Match::warning("No exact English name or printed passcode matched the catalogue.");
    };
    if !read.passcodes.is_empty() && !read.passcodes.contains(&card.id) {
        return Match::warning(
            "Printed passcode conflicts with the name match; no card was selected.",
        );
    }
    printing(card, &read, !by_name.is_empty())
}
fn printing(card: &Card, read: &text::Evidence, named: bool) -> Match {
    let mut groups: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for printing in card
        .card_sets
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|p| read.codes.contains(&p.set_code.to_uppercase()))
    {
        groups
            .entry(printing.set_code.to_uppercase())
            .or_default()
            .push(printing);
    }
    if groups.len() > 1 {
        return Match::warning("Multiple printed set codes matched; review the saved image.");
    }
    let (code, variants) = groups.into_iter().next().unwrap_or_default();
    let mut evidence = vec![];
    if read.passcodes.contains(&card.id) {
        evidence.push(format!("Printed passcode {:08}", card.id));
    }
    if named {
        evidence.push(format!("Exact printed name {}", card.name));
    }
    let mut warnings=vec!["Catalogue rarity describes possible printings, not a measured foil finish or condition grade.".into()];
    if code.is_empty() {
        warnings.push("Printed set code is unreadable or not listed for this card; printing remains unresolved.".into());
    } else {
        evidence.push(format!("Printed set code {code}"));
    }
    let names: BTreeSet<_> = variants.iter().map(|v| v.set_name.as_str()).collect();
    let rarities: BTreeSet<_> = variants.iter().map(|v| v.set_rarity.as_str()).collect();
    if rarities.len() > 1 {
        warnings.push("This set code has several catalogue rarities; the physical variant remains unresolved.".into());
    }
    let mut extensions = scanner_types::Extensions::new();
    extensions.insert("provider".into(), serde_json::json!("ygoprodeck"));
    extensions.insert("catalogueID".into(), serde_json::json!(card.id.to_string()));
    extensions.insert("rarities".into(), serde_json::json!(rarities));
    let score = evidence.len();
    let candidate = CardCandidate {
        id: format!(
            "ygoprodeck:{}:{}",
            card.id,
            if code.is_empty() { "unresolved" } else { &code }
        ),
        name: card.name.clone(),
        local_id: code.clone(),
        set_id: code,
        set_name: if names.is_empty() {
            "Printing not identified".into()
        } else {
            names.into_iter().collect::<Vec<_>>().join(" / ")
        },
        official_set_count: None,
        evidence,
        category: Some(card.category.clone()),
        types: None,
        available_finishes: vec![],
        extensions,
    };
    Match {
        candidates: vec![candidate],
        score,
        warnings,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn lines(name: &str, code: &str, passcode: &str) -> Vec<RecognizedLine> {
        [(name, 0.9), (code, 0.3), (passcode, 0.02)]
            .into_iter()
            .map(|(text, y)| RecognizedLine {
                text: text.into(),
                confidence: 0.9,
                x: 0.,
                y,
                width: 0.8,
                height: 0.05,
            })
            .collect()
    }
    #[test]
    fn independent_identity_fields_must_agree_and_rarity_does_not_imply_foil() {
        let mut card:Card=serde_json::from_value(serde_json::json!({"id":39706423,"name":"GMX Suppression Squad","type":"Effect Monster","card_sets":[{"set_name":"Blazing Dominion","set_code":"BLZD-EN091","set_rarity":"Common"}]})).unwrap();
        let good = lines("GMX Suppression Squad", "BLZD-EN091", "39706423");
        let matched = match_card(&good, std::slice::from_ref(&card));
        assert_eq!(matched.candidates[0].id, "ygoprodeck:39706423:BLZD-EN091");
        assert!(matched.candidates[0].available_finishes.is_empty());
        assert_eq!(
            matched.candidates[0].extensions["rarities"],
            serde_json::json!(["Common"])
        );
        assert!(
            match_card(
                &lines("GMX Suppression Squad", "BLZD-EN091", "12345678"),
                std::slice::from_ref(&card)
            )
            .candidates
            .is_empty()
        );
        assert!(
            match_card(
                &lines("Unreadable", "BLZD-EN091", ""),
                std::slice::from_ref(&card)
            )
            .candidates
            .is_empty()
        );
        assert_eq!(
            match_card(
                &lines("GMX Suppression Squad", "UNKNOWN-999", "39706423"),
                std::slice::from_ref(&card)
            )
            .candidates[0]
                .local_id,
            ""
        );
        for (name, passcode) in [("GMX Suppression Squad", ""), ("", "39706423")] {
            assert_eq!(
                match_card(
                    &lines(name, "BLZD-EN091", passcode),
                    std::slice::from_ref(&card)
                )
                .candidates
                .len(),
                1
            );
        }
        let mut low = good.clone();
        for line in &mut low {
            line.confidence = 0.2;
        }
        assert!(
            match_card(&low, std::slice::from_ref(&card))
                .candidates
                .is_empty()
        );
        card.card_sets
            .as_mut()
            .unwrap()
            .push(scanner_catalogue::yugioh::Printing {
                set_name: "Blazing Dominion".into(),
                set_code: "BLZD-EN091".into(),
                set_rarity: "Ultra Rare".into(),
            });
        let matched = match_card(&good, &[card]);
        assert_eq!(
            matched.candidates[0].extensions["rarities"],
            serde_json::json!(["Common", "Ultra Rare"])
        );
    }
}
