use super::text::{bounded, normalized, number};
use crate::ocr::RecognizedLine;
use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::LazyLock,
};

static FRACTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)([A-Z]{0,5}[0-9]{1,4})\s*/\s*([A-Z]{0,5}[0-9]{1,4})").unwrap()
});
static COLLECTOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\s*[A-Z]{0,5}[0-9]{1,4}\s*/\s*[A-Z]{0,5}[0-9]{1,4}(?:\s*[^A-Z0-9/\s]+|\s+[O0])?\s*$",
    )
    .unwrap()
});
static PROMO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:SWSH|SM|XY|BW|SVP|DP|HGSS)[0-9]{1,4}\b").unwrap());
static SET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)((?:SV|SM|XY|BW|S)[0-9]{1,3}[A-Z]{0,3}\+?)").unwrap());
pub(super) const SUFFIXES: [&str; 6] = ["ex", "gx", "v", "vmax", "vstar", "break"];

pub(super) struct Evidence {
    pub titles: Vec<String>,
    pub numbers: BTreeSet<String>,
    pub totals: BTreeMap<String, BTreeSet<u32>>,
    pub energy_title: bool,
    pub energy_sets: BTreeSet<String>,
    pub standalone: BTreeSet<String>,
    pub sets: BTreeSet<String>,
    pub suffixes: BTreeSet<String>,
}

pub fn footer_orientation_score(lines: &[RecognizedLine]) -> u8 {
    u8::from(lines.iter().any(|line| collector(line) && line.y < 0.30))
}

fn collector(line: &RecognizedLine) -> bool {
    line.confidence >= 0.35 && line.height <= 0.10 && COLLECTOR.is_match(&line.text)
}

impl Evidence {
    pub fn read(lines: &[RecognizedLine]) -> Option<Self> {
        if lines.iter().any(|line| collector(line) && line.y > 0.65) {
            return None;
        }
        let readable: Vec<_> = lines
            .iter()
            .filter(|line| line.confidence >= 0.35)
            .collect();
        let titles: Vec<_> = readable
            .iter()
            .filter(|line| line.y + line.height > 0.65 && line.height < 0.15)
            .map(|line| format!(" {} ", normalized(&line.text)))
            .collect();
        let footers: Vec<_> = readable
            .iter()
            .filter(|line| line.y < 0.30 && line.height <= 0.10)
            .map(|line| line.text.as_str())
            .collect();
        let (numbers, totals) = numbers(&footers);
        let energy_sets = footers
            .iter()
            .filter_map(|text| {
                let code = normalized(text).replace(' ', "");
                ["sve", "mee"]
                    .into_iter()
                    .find(|set| code == *set || code == format!("{set}en"))
                    .map(str::to_owned)
            })
            .collect();
        let standalone = footers
            .iter()
            .filter_map(|text| standalone_number(text))
            .collect();
        let sets = footers
            .iter()
            .flat_map(|text| {
                SET.find_iter(text)
                    .filter(|m| bounded(text, m.start(), m.end()))
                    .map(|m| m.as_str().to_uppercase())
            })
            .collect();
        let suffixes = titles
            .iter()
            .flat_map(|title| title.split_whitespace())
            .filter(|word| SUFFIXES.contains(word))
            .map(str::to_owned)
            .collect();
        let energy_title = titles.iter().any(|title| title.contains(" energy "));
        Some(Self {
            titles,
            numbers,
            totals,
            energy_title,
            energy_sets,
            standalone,
            sets,
            suffixes,
        })
    }
}

fn numbers(footers: &[&str]) -> (BTreeSet<String>, BTreeMap<String, BTreeSet<u32>>) {
    let mut numbers = BTreeSet::new();
    let mut totals: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
    for text in footers {
        for capture in FRACTION.captures_iter(text) {
            let whole = capture.get(0).unwrap();
            if !bounded(text, whole.start(), whole.end()) {
                continue;
            }
            let key = number(&capture[1]);
            numbers.insert(key.clone());
            if let Ok(total) = capture[2].parse() {
                totals.entry(key).or_default().insert(total);
            }
        }
        numbers.extend(PROMO.find_iter(text).map(|m| number(m.as_str())));
    }
    (numbers, totals)
}

fn standalone_number(text: &str) -> Option<String> {
    let text = text.trim();
    if text.len() < 3 || !text.as_bytes()[..3].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let rest = text[3..].trim_start().to_lowercase();
    (rest.is_empty() || rest.starts_with("illus.") || rest.starts_with("lllus."))
        .then(|| number(&text[..3]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn does_not_trim_ocr_digits_to_fit_a_catalogue_total() {
        let (numbers, totals) = numbers(&[
            "028/1590",
            "TG01/TG30",
            "12345/159",
            "009 Illus. YOSHIROTTEN",
        ]);
        assert_eq!(totals["28"], BTreeSet::from([1590]));
        assert!(numbers.contains("TG1"));
        assert!(!numbers.contains("2345"));
        assert_eq!(
            standalone_number("009 Illus. YOSHIROTTEN"),
            Some("9".into())
        );
    }
}
