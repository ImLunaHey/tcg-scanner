use std::{collections::BTreeSet, sync::OnceLock};
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Identifier {
    pub set: String,
    pub number: u32,
    pub total: Option<u32>,
}
/// Parse complete English footer identifiers from one OCR pass. Never combine
/// independent OCR passes or infer a set from a number alone.
pub fn identifiers<'a>(lines: impl IntoIterator<Item = &'a str>) -> BTreeSet<Identifier> {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    let text = lines
        .into_iter()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase();
    let pattern=PATTERN.get_or_init(||regex::Regex::new(r"\b([A-Z][A-Z0-9]{2,3})\s*[•·.\-]?\s*EN\b[^0-9/]{0,12}\b([0-9]{1,3})(?:\s*/\s*([0-9]{3}))?").unwrap());
    pattern
        .captures_iter(&text)
        .filter_map(|c| {
            let rest = &text[c.get(0)?.end()..];
            if rest
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
                || rest.trim_start().starts_with('/')
            {
                return None;
            }
            Some(Identifier {
                set: c[1].into(),
                number: c[2].parse().ok()?,
                total: match c.get(3) {
                    Some(v) => Some(v.as_str().parse().ok()?),
                    None => None,
                },
            })
        })
        .collect()
}
pub fn is_card<'a>(words: impl IntoIterator<Item = &'a str>) -> bool {
    let text = words
        .into_iter()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase();
    text.contains("LFL")
        && text.contains("FFG")
        && (!identifiers([text.as_str()]).is_empty()
            || (text.contains("TOKEN") && text.contains("EN")))
}
pub fn is_leader_unit<'a>(words: impl IntoIterator<Item = &'a str>) -> bool {
    let text = words
        .into_iter()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase();
    text.contains("LEADER UNIT") && text.contains("OVERWHELM") && text.contains("SENTINEL")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identifiers_require_language_and_do_not_accept_truncated_totals() {
        let ids = identifiers(["ASH•EN R 206/264"]);
        assert_eq!(
            ids.into_iter().next().unwrap(),
            Identifier {
                set: "ASH".into(),
                number: 206,
                total: Some(264)
            }
        );
        for text in [
            "ASH 206/264",
            "ASH EN 206/2640",
            "ASH FR 206/264",
            "ASH EN 206 /oops",
            "ASH EN 206A/264",
            "ASH EN 1234",
        ] {
            assert!(identifiers([text]).is_empty(), "{text}");
        }
        assert_eq!(
            identifiers(["ASH EN 686"])
                .into_iter()
                .next()
                .unwrap()
                .number,
            686
        );
        assert_eq!(identifiers(["ASH EN 206/264", "SOR EN 20/252"]).len(), 2);
        assert!(is_card(["LFL FFG ASH EN 206/264"]));
        assert!(!is_card(["ASH EN 206/264"]));
    }
}
