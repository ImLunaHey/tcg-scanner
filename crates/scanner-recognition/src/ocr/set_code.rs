use super::RecognizedLine;
/// Tesseract may split the set code and language into adjacent words. Retain
/// their exact union instead of including a neighbouring regulation symbol.
pub(super) fn joined_words(words: &[RecognizedLine]) -> Vec<RecognizedLine> {
    let mut joined = vec![];
    for code in words {
        let text = code.text.trim().to_uppercase();
        if text.len() != 3 || !text.bytes().all(|b| b.is_ascii_alphanumeric()) {
            continue;
        }
        for language in words {
            if !language.text.trim().eq_ignore_ascii_case("EN") {
                continue;
            }
            let gap = language.x - code.x - code.width;
            let overlap =
                (code.y + code.height).min(language.y + language.height) - code.y.max(language.y);
            if !(-0.002..=0.02).contains(&gap) || overlap < code.height.min(language.height) * 0.5 {
                continue;
            }
            let x = code.x.min(language.x);
            let y = code.y.min(language.y);
            joined.push(RecognizedLine {
                text: format!("{text} EN"),
                confidence: code.confidence.min(language.confidence),
                x,
                y,
                width: (code.x + code.width).max(language.x + language.width) - x,
                height: (code.y + code.height).max(language.y + language.height) - y,
            });
        }
    }
    joined
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_neighbouring_aligned_language_words_extend_the_code() {
        let code = RecognizedLine {
            text: "SVI".into(),
            confidence: 0.9,
            x: 0.06,
            y: 0.03,
            width: 0.045,
            height: 0.02,
        };
        let mut language = RecognizedLine {
            text: "EN".into(),
            x: 0.11,
            width: 0.02,
            ..code.clone()
        };
        let joined = joined_words(&[code.clone(), language.clone()]);
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].text, "SVI EN");
        assert!((joined[0].width - 0.07).abs() < 1e-10);
        language.y = 0.1;
        assert!(joined_words(&[code.clone(), language.clone()]).is_empty());
        language.y = code.y;
        language.x = 0.2;
        assert!(joined_words(&[code, language]).is_empty());
    }
}
