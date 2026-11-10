use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

pub fn normalized(text: &str) -> String {
    let plain: String = text.nfd().filter(|c| !is_combining_mark(*c)).collect();
    plain
        .nfc()
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn number(text: &str) -> String {
    let value = normalized(text).replace(' ', "").to_uppercase();
    let start = value
        .find(|c: char| c.is_ascii_digit())
        .unwrap_or(value.len());
    let (prefix, suffix) = value.split_at(start);
    format!(
        "{prefix}{}",
        suffix
            .parse::<u64>()
            .map(|n| n.to_string())
            .unwrap_or_else(|_| suffix.into())
    )
}

pub(super) fn bounded(text: &str, start: usize, end: usize) -> bool {
    let alnum = |c: char| c.is_ascii_alphanumeric();
    !text[..start].chars().next_back().is_some_and(alnum)
        && !text[end..].chars().next().is_some_and(alnum)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalization_preserves_hangul_and_number_prefixes() {
        assert_eq!(normalized("Farfetch’d"), "farfetch d");
        assert_eq!(normalized("Pokémon"), "pokemon");
        assert_eq!(normalized("야나프"), "야나프");
        assert_eq!(number("TG01"), "TG1");
        assert_eq!(number("001"), "1");
        assert_ne!(number("TG01"), number("001"));
    }
}
