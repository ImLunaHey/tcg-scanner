use crate::{matching::text::normalized, ocr::RecognizedLine};
use regex::Regex;
use std::sync::LazyLock;

pub(super) fn letters(text: &str) -> String {
    normalized(text)
        .chars()
        .filter(|c| c.is_alphabetic())
        .collect()
}
pub(super) fn resembles(text: &str, target: &str, distance: usize) -> bool {
    let text: Vec<_> = letters(text).chars().collect();
    let target: Vec<_> = target.chars().collect();
    if text.len().abs_diff(target.len()) > distance {
        return false;
    }
    let mut previous: Vec<_> = (0..=target.len()).collect();
    for (row, letter) in text.iter().enumerate() {
        let mut next = vec![row + 1];
        for (column, expected) in target.iter().enumerate() {
            next.push(
                (previous[column + 1] + 1)
                    .min(next[column] + 1)
                    .min(previous[column] + usize::from(letter != expected)),
            );
        }
        previous = next;
    }
    previous[target.len()] <= distance
}

pub(super) fn yugioh(lines: &[RecognizedLine]) -> bool {
    let konami = lines.iter().any(|l| {
        l.confidence >= 0.4
            && letters(&l.text).contains("konami")
            && l.y + l.height / 2. > 0.75
            && l.x + l.width / 2. < 0.55
    });
    let logo = lines.iter().any(|l| {
        l.confidence >= 0.3
            && l.y + l.height / 2. < 0.25
            && (letters(&l.text).contains("yugioh") || resembles(&l.text, "tradingcardgame", 1))
    });
    konami && logo
}
pub(super) fn starwars(lines: &[RecognizedLine]) -> bool {
    let readable: Vec<_> = lines.iter().filter(|l| l.confidence >= 0.5).collect();
    readable.len() <= 4
        && readable.iter().any(|l| {
            letters(&l.text) == "unlimited"
                && (0.24..=0.44).contains(&(l.y + l.height / 2.))
                && (0.35..=0.65).contains(&(l.x + l.width / 2.))
                && l.width > 0.5
        })
}
pub(super) fn energy(lines: &[RecognizedLine]) -> bool {
    let title = lines.iter().any(|l| {
        l.confidence >= 0.7
            && l.y + l.height / 2. > 0.8
            && l.x + l.width / 2. < 0.8
            && matches!(letters(&l.text).as_str(), "energy" | "basicenergy")
    });
    let footer = lines.iter().any(|l| {
        let text = normalized(&l.text);
        l.confidence >= 0.5
            && l.y + l.height / 2. < 0.12
            && text.contains("pokemon")
            && ["nintendo", "creatures", "game freak"]
                .iter()
                .any(|s| text.contains(s))
    });
    title && footer
}

static REDEEM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:https?://)?(?:www\.)?pokemon\s*\.\s*com\s*/\s*(?:redeem|tcgo)").unwrap()
});
fn redemption_url(text: &str) -> bool {
    REDEEM.find_iter(text).any(|m| {
        let before = text[..m.start()].chars().next_back();
        let after = &text[m.end()..];
        !before.is_some_and(|c| c.is_ascii_alphanumeric() || ".-/".contains(c))
            && !after
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || "/_-".contains(c))
            && !(after.starts_with('.')
                && after
                    .chars()
                    .nth(1)
                    .is_some_and(|c| c.is_ascii_alphanumeric()))
    })
}
pub(super) fn code_card(lines: &[RecognizedLine]) -> Option<&'static str> {
    let words: Vec<_> = lines
        .iter()
        .filter(|l| l.confidence >= 0.7)
        .map(|l| l.text.as_str())
        .collect();
    if redemption_url(&words.join(" ")) {
        return Some("pokemon-code-url");
    }
    let labels: Vec<_> = words.into_iter().map(letters).collect();
    (labels.iter().any(|s| s == "live") && labels.iter().any(|s| s == "tradingcardgame"))
        .then_some("pokemon-code-text")
}
pub(super) fn score(lines: &[RecognizedLine]) -> (usize, f64) {
    lines
        .iter()
        .filter(|l| l.confidence >= 0.5 && l.text.chars().count() >= 4)
        .fold((0, 0.), |(count, score), l| {
            (
                count + 1,
                score + f64::from(l.confidence) * l.text.chars().count().min(24) as f64,
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn code_urls_require_exact_public_host_and_path() {
        for url in [
            "https://www.pokemon.com/redeem",
            "pokemon . com / tcgo",
            "(pokemon.com/redeem).",
        ] {
            assert!(redemption_url(url));
        }
        for url in [
            "evilpokemon.com/redeem",
            "bad.pokemon.com/redeem",
            "pokemon.com/redeem.evil",
            "pokemon.com/redeem123",
            "pokemon.com/redeem/other",
        ] {
            assert!(!redemption_url(url), "{url}");
        }
    }
    #[test]
    fn fuzzy_logos_have_bounded_edits() {
        assert!(resembles("Pokémon", "pokemon", 2));
        assert!(resembles("Pokemcn", "pokemon", 2));
        assert!(!resembles("Pokemon Trading", "pokemon", 2));
        assert!(!resembles("Pikachu", "pokemon", 2));
    }
}
