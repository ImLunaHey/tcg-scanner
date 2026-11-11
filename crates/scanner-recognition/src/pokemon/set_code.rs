//! Modern English footer abbreviations, constrained by catalogue metadata.
use regex::Regex;
use scanner_types::{catalogue::SetAbbreviation, recognition::RecognizedLine};
use std::{collections::BTreeSet, sync::LazyLock};
static CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b([A-Z0-9]{3})\s*EN\b").unwrap());

pub(super) fn read<'a>(lines: impl Iterator<Item = &'a RecognizedLine>) -> BTreeSet<String> {
    lines
        .filter(|l| l.confidence >= 0.7 && l.y < 0.15 && l.height < 0.1)
        .flat_map(|l| CODE.captures_iter(&l.text).map(|c| c[1].to_uppercase()))
        .collect()
}

pub(super) fn agrees(
    codes: &BTreeSet<String>,
    abbreviation: Option<&SetAbbreviation>,
) -> Option<String> {
    if codes.len() != 1 {
        return None;
    }
    let code = abbreviation?.official.as_ref()?.trim().to_uppercase();
    codes.contains(&code).then_some(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codes_require_confident_footer_text_and_language() {
        let mut line = RecognizedLine {
            text: "CRI EN".into(),
            confidence: 0.95,
            x: 0.04,
            y: 0.04,
            width: 0.15,
            height: 0.03,
        };
        assert_eq!(read([&line].into_iter()), BTreeSet::from(["CRI".into()]));
        line.y = 0.8;
        assert!(read([&line].into_iter()).is_empty());
        line.y = 0.04;
        line.confidence = 0.6;
        assert!(read([&line].into_iter()).is_empty());
        line.confidence = 0.95;
        line.text = "CRI".into();
        assert!(read([&line].into_iter()).is_empty());
    }
    #[test]
    fn unknown_or_conflicting_codes_cannot_confirm_a_printing() {
        let known = SetAbbreviation {
            official: Some("CRI".into()),
        };
        assert!(agrees(&BTreeSet::from(["CRI".into()]), Some(&known)).is_some());
        assert!(agrees(&BTreeSet::from(["PAL".into()]), Some(&known)).is_none());
        assert!(agrees(&BTreeSet::from(["CRI".into(), "PAL".into()]), Some(&known)).is_none());
        assert!(agrees(&BTreeSet::from(["CRI".into()]), None).is_none());
    }
}
