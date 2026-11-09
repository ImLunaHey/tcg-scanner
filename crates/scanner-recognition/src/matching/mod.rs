mod evidence;
pub mod text;
pub use evidence::footer_orientation_score;

use crate::ocr::RecognizedLine;
use evidence::{Evidence, SUFFIXES};
use scanner_types::cards::CardLanguage;
use serde::Serialize;
use std::collections::BTreeSet;

pub use scanner_types::catalogue::CatalogueCard;

#[derive(Clone, Debug, Serialize)]
pub struct Ranked {
    pub card: CatalogueCard,
    pub score: usize,
    pub evidence: Vec<String>,
    pub totals: BTreeSet<u32>,
}

pub struct PokemonMatcher {
    cards: Vec<IndexedCard>,
    language: CardLanguage,
}
struct IndexedCard {
    card: CatalogueCard,
    name: String,
    local: String,
    set: String,
}

impl PokemonMatcher {
    pub fn new(cards: Vec<CatalogueCard>, language: CardLanguage) -> Self {
        let cards = cards
            .into_iter()
            .filter(|card| {
                !card
                    .image
                    .as_deref()
                    .is_some_and(|url| url.contains("/tcgp/"))
            })
            .map(|card| IndexedCard {
                name: text::normalized(&card.name),
                local: text::number(&card.local_id),
                set: card
                    .id
                    .rsplit_once('-')
                    .map(|(s, _)| s)
                    .unwrap_or("")
                    .into(),
                card,
            })
            .collect();
        Self { cards, language }
    }
    pub fn rank(&self, lines: &[RecognizedLine]) -> Vec<Ranked> {
        let Some(evidence) = Evidence::read(lines) else {
            return vec![];
        };
        let mut results: Vec<_> = self
            .cards
            .iter()
            .filter_map(|card| rank_card(card, &evidence, self.language))
            .collect();
        results.sort_by(|a, b| b.score.cmp(&a.score).then(a.card.id.cmp(&b.card.id)));
        results
    }
}

fn rank_card(index: &IndexedCard, reading: &Evidence, language: CardLanguage) -> Option<Ranked> {
    let IndexedCard {
        card,
        name,
        local,
        set,
    } = index;
    if name.is_empty() {
        return None;
    }
    let energy = reading.energy_title
        && reading.energy_sets.len() == 1
        && reading.standalone.len() == 1
        && reading.standalone.contains(local)
        && reading.energy_sets.contains(set);
    if reading.energy_title && !reading.energy_sets.is_empty() && !energy {
        return None;
    }
    let numbered = reading.numbers.contains(local) || energy;
    let korean = language == CardLanguage::Korean;
    let hangul = name.chars().any(|c| ('\u{ac00}'..='\u{d7a3}').contains(&c));
    if name.chars().count() < 3 && !(korean && hangul && numbered) || korean && !numbered {
        return None;
    }
    if korean && !korean_printing_agrees(name, set, reading) {
        return None;
    }
    let exact = reading
        .titles
        .iter()
        .any(|title| title.contains(&format!(" {name} ")));
    let uncertain = !korean
        && numbered
        && name.strip_suffix(" ex").is_some_and(|base| {
            reading.titles.iter().any(|title| {
                ["x", "cx"]
                    .iter()
                    .any(|suffix| title.contains(&format!(" {base} {suffix} ")))
            })
        });
    if !(exact || uncertain || energy) || !reading.numbers.is_empty() && !numbered {
        return None;
    }
    let evidence = if energy {
        vec![
            "Energy title".into(),
            format!("Printed set code {}", set.to_uppercase()),
            format!("Collector number {}", card.local_id),
        ]
    } else if uncertain {
        vec![
            "Name with uncertain ex suffix; collector number agrees".into(),
            format!("Collector number {}", card.local_id),
        ]
    } else if numbered {
        vec![
            "Name in title area".into(),
            format!("Collector number {}", card.local_id),
        ]
    } else {
        vec!["Name in title area only; printing is ambiguous".into()]
    };
    Some(Ranked {
        card: card.clone(),
        score: usize::from(numbered) * 100 + name.chars().count(),
        evidence,
        totals: reading.totals.get(local).cloned().unwrap_or_default(),
    })
}

fn korean_printing_agrees(name: &str, set: &str, reading: &Evidence) -> bool {
    if !reading.sets.is_empty() && reading.sets != BTreeSet::from([set.to_uppercase()]) {
        return false;
    }
    let expected: BTreeSet<_> = name
        .split_whitespace()
        .last()
        .filter(|s| SUFFIXES.contains(s))
        .map(str::to_owned)
        .into_iter()
        .collect();
    reading.suffixes.is_empty() || reading.suffixes == expected
}

pub fn agreeing_footer_ranks(first: &[Ranked], second: &[Ranked]) -> Vec<Ranked> {
    first
        .iter()
        .filter(|a| {
            a.totals.len() == 1
                && second
                    .iter()
                    .any(|b| a.card.id == b.card.id && a.totals == b.totals)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_native_candidates_survive_the_ported_evidence_rules() {
        let Some(directory) = std::env::var_os("SCANNER_BENCH_DIR") else {
            return;
        };
        let directory = std::path::PathBuf::from(directory);
        let cards: Vec<CatalogueCard> =
            serde_json::from_slice(&std::fs::read(directory.join("catalogue.json")).unwrap())
                .unwrap();
        let cases: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(directory.join("native.json")).unwrap()).unwrap();
        let matcher = PokemonMatcher::new(cards, CardLanguage::English);
        let mut checked = 0;
        for case in cases {
            let lines: Vec<RecognizedLine> =
                serde_json::from_value(case["evidence"]["lines"].clone()).unwrap();
            let ranks = matcher.rank(&lines);
            for candidate in case["evidence"]["candidates"].as_array().unwrap() {
                assert!(
                    ranks.iter().any(|rank| rank.card.id == candidate["id"]),
                    "Lost candidate in {}",
                    case["id"]
                );
                checked += 1;
            }
        }
        assert!(checked >= 62);
    }
    fn card(id: &str, number: &str, name: &str) -> CatalogueCard {
        CatalogueCard {
            id: id.into(),
            local_id: number.into(),
            name: name.into(),
            image: None,
        }
    }
    fn line(text: &str, y: f64) -> RecognizedLine {
        RecognizedLine {
            text: text.into(),
            confidence: 1.,
            x: 0.1,
            y,
            width: 0.5,
            height: 0.04,
        }
    }
    #[test]
    fn reprints_stay_ambiguous_and_conflicting_numbers_reject() {
        let matcher = PokemonMatcher::new(
            vec![
                card("a-136", "136", "Furret"),
                card("b-136", "136", "Furret"),
                card("c-12", "12", "Furret"),
            ],
            CardLanguage::English,
        );
        let result = matcher.rank(&[line("STAGE 1 Furret HP 110", 0.88), line("136/189", 0.04)]);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|r| r.totals == BTreeSet::from([189])));
        assert!(
            matcher
                .rank(&[line("Furret", 0.9), line("99/100", 0.04)])
                .is_empty()
        );
        assert!(matcher.rank(&[line("SuperFurret", 0.9)]).is_empty());
        assert!(matcher.rank(&[line("Furret", 0.3)]).is_empty());
    }
    #[test]
    fn inverted_collector_cannot_promote_flavour_text() {
        let matcher = PokemonMatcher::new(vec![card("a-6", "6", "Groudon")], CardLanguage::English);
        let lines = [
            line("Kyogre", 0.05),
            line("battled ferociously against Groudon.", 0.94),
            line("026/103", 0.94),
        ];
        assert!(matcher.rank(&lines).is_empty());
        assert_eq!(footer_orientation_score(&lines), 0);
    }
    #[test]
    fn energy_requires_unique_set_and_number() {
        let matcher = PokemonMatcher::new(
            vec![
                card("mee-009", "009", "Basic Grass Energy"),
                card("sve-009", "009", "Basic Grass Energy"),
            ],
            CardLanguage::English,
        );
        let lines = [
            line("Basic Energy", 0.9),
            line("MEE EN", 0.04),
            line("009 Illus. YOSHIROTTEN", 0.04),
        ];
        assert_eq!(matcher.rank(&lines)[0].card.id, "mee-009");
        assert!(matcher.rank(&lines[..2]).is_empty());
        let mut conflict = lines.to_vec();
        conflict.push(line("008", 0.03));
        assert!(matcher.rank(&conflict).is_empty());
    }
    #[test]
    fn korean_requires_number_and_preserves_set_and_class() {
        let matcher = PokemonMatcher::new(
            vec![
                card("SV4K-001", "001", "뮤"),
                card("SV4K-002", "002", "뮤 ex"),
            ],
            CardLanguage::Korean,
        );
        assert!(matcher.rank(&[line("뮤", 0.9)]).is_empty());
        assert_eq!(
            matcher.rank(&[line("뮤", 0.9), line("001/066", 0.03)])[0]
                .card
                .id,
            "SV4K-001"
        );
        assert!(
            matcher
                .rank(&[line("뮤", 0.9), line("001/066", 0.03), line("SV5K", 0.03)])
                .is_empty()
        );
        assert!(
            matcher
                .rank(&[line("뮤 ex", 0.9), line("001/066", 0.03)])
                .is_empty()
        );
    }
    #[test]
    fn uncertain_ex_suffix_requires_number_and_footer_retries_must_agree() {
        let matcher = PokemonMatcher::new(
            vec![card("sv09-177", "177", "Salamence ex")],
            CardLanguage::English,
        );
        let first = matcher.rank(&[line("Salamence @X", 0.9), line("177/159", 0.03)]);
        assert_eq!(first[0].card.id, "sv09-177");
        assert!(matcher.rank(&[line("Salamence @X", 0.9)]).is_empty());
        let second = matcher.rank(&[line("Salamence @X", 0.9), line("177/1590", 0.03)]);
        assert!(agreeing_footer_ranks(&first, &second).is_empty());
    }
}
