use scanner_types::recognition::RecognizedLine;
use std::{collections::BTreeSet, sync::OnceLock};
pub(super) struct Evidence {
    pub titles: BTreeSet<String>,
    pub passcodes: BTreeSet<u64>,
    pub codes: BTreeSet<String>,
}
pub(super) fn normalized(text: &str) -> String {
    crate::matching::text::normalized(text).replace(' ', "")
}
pub(super) fn read(lines: &[RecognizedLine]) -> Evidence {
    static PASSCODE: OnceLock<regex::Regex> = OnceLock::new();
    static CODE: OnceLock<regex::Regex> = OnceLock::new();
    let passcode = PASSCODE.get_or_init(|| regex::Regex::new(r"\b[0-9]{8}\b").unwrap());
    let code = CODE.get_or_init(|| {
        regex::Regex::new(r"\b[A-Z0-9]{2,6}\s*[-‐‑–]\s*(?:EN|AE)?[0-9]{2,4}\b").unwrap()
    });
    let mut result = Evidence {
        titles: BTreeSet::new(),
        passcodes: BTreeSet::new(),
        codes: BTreeSet::new(),
    };
    for line in lines.iter().filter(|l| l.confidence >= 0.5) {
        if line.y > 0.78 {
            let title = normalized(&line.text);
            if title.chars().count() >= 4 {
                result.titles.insert(title);
            }
        }
        if line.y < 0.15 {
            result.passcodes.extend(
                passcode
                    .find_iter(&line.text)
                    .filter_map(|m| m.as_str().parse::<u64>().ok()),
            );
        }
        if line.y < 0.5 {
            result
                .codes
                .extend(code.find_iter(&line.text.to_uppercase()).map(|m| {
                    m.as_str()
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .map(|c| {
                            if ['‐', '‑', '–'].contains(&c) {
                                '-'
                            } else {
                                c
                            }
                        })
                        .collect::<String>()
                }));
        }
    }
    result
}
