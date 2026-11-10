use crate::matching::text::normalized;
use regex::Regex;
use scanner_types::{printed::UncataloguedCard, recognition::RecognizedLine};
use std::{collections::BTreeSet, sync::LazyLock};

static CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b([A-Z0-9]{3})\s+EN\b").unwrap());
static NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*([A-Z]{0,5}[0-9]{1,4})\s*/\s*([0-9]{1,4})\b").unwrap());
static HP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*(?:HP\s*\d+|\d+\s*HP)").unwrap());
static END_HP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s+(?:HP\s*\d+|\d+\s*HP)\s*$").unwrap());

/// Transcribe only when one clear set code and fraction agree within a single
/// independent reading. Never synthesize a catalogue ID or repair OCR digits.
pub(super) fn read(lines: &[RecognizedLine]) -> Option<UncataloguedCard> {
    let footer = lines
        .iter()
        .filter(|l| l.confidence >= 0.7 && l.y < 0.15 && l.height < 0.1);
    let codes: BTreeSet<_> = footer
        .clone()
        .flat_map(|l| CODE.captures_iter(&l.text))
        .map(|c| c[1].to_uppercase())
        .collect();
    let numbers: BTreeSet<_> = footer
        .flat_map(|l| NUMBER.captures_iter(&l.text))
        .map(|c| format!("{}/{}", c[1].to_uppercase(), &c[2]))
        .collect();
    if codes.len() != 1 || numbers.len() != 1 {
        return None;
    }
    let title = lines
        .iter()
        .filter(|l| {
            l.confidence >= 0.7
                && l.y > 0.8
                && l.height > 0.025
                && l.height < 0.12
                && l.text.chars().any(char::is_alphabetic)
                && !HP.is_match(&l.text)
                && ![
                    "basic",
                    "stage 1",
                    "stage 2",
                    "stage1",
                    "stage2",
                    "trainer",
                    "energy",
                    "hp",
                    "supporter",
                    "item",
                    "stadium",
                    "pokemon tool",
                ]
                .contains(&normalized(&l.text).as_str())
                && !l.text.to_lowercase().starts_with("evolves from")
        })
        .max_by(|a, b| a.height.total_cmp(&b.height))?;
    let name = END_HP.replace(&title.text, "").trim().to_owned();
    if !(2..=100).contains(&name.chars().count()) {
        return None;
    }
    Some(UncataloguedCard {
        name,
        set_code: codes.into_iter().next()?,
        collector_number: numbers.into_iter().next()?,
        // A printed code alone cannot establish a user-confirmed set label.
        set_name: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn line(text: &str, y: f64) -> RecognizedLine {
        RecognizedLine {
            text: text.into(),
            confidence: 0.95,
            x: 0.1,
            y,
            width: 0.4,
            height: 0.04,
        }
    }
    #[test]
    fn transcribes_without_inventing_a_catalogue_identity_or_set_name() {
        let lines = vec![
            line("Salamence ex 330 HP", 0.89),
            line("DLR EN", 0.04),
            line("177/159", 0.04),
        ];
        let result = read(&lines).unwrap();
        assert_eq!(result.name, "Salamence ex");
        assert_eq!(result.set_code, "DLR");
        assert_eq!(result.collector_number, "177/159");
        assert_eq!(result.set_name, None);
        let mut conflict = lines.clone();
        conflict.push(line("178/159", 0.04));
        assert!(read(&conflict).is_none());
        let mut conflict = lines.clone();
        conflict.push(line("JTG EN", 0.04));
        assert!(read(&conflict).is_none());
        for text in ["177/15900", "177/159X", "177/", "177159"] {
            let mut invalid = lines.clone();
            invalid[2].text = text.into();
            assert!(read(&invalid).is_none(), "{text}");
        }
        let mut weak = lines;
        weak[1].confidence = 0.69;
        assert!(read(&weak).is_none());
    }
}
