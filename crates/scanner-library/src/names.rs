use crate::Inventory;
use std::collections::{BTreeMap, BTreeSet};
impl Inventory {
    pub(crate) fn slugs(&self) -> BTreeMap<String, String> {
        let mut groups = BTreeMap::new();
        for entry in self.entries.values() {
            groups.entry(entry.set_key.clone()).or_insert((
                slug(entry.set_name.as_deref().unwrap_or("Unidentified")),
                entry.game,
            ));
        }
        let mut counts = BTreeMap::new();
        for (name, _) in groups.values() {
            *counts.entry(name.clone()).or_insert(0) += 1;
        }
        let mut used = BTreeSet::new();
        let mut result = BTreeMap::new();
        for (key, (name, game)) in groups {
            let base = if counts[&name] == 1 {
                name
            } else {
                format!("{name}-{}", slug(game.as_str()))
            };
            let mut candidate = base.clone();
            let mut suffix = 2;
            while !used.insert(candidate.clone()) {
                candidate = format!("{base}-{suffix}");
                suffix += 1;
            }
            result.insert(key, candidate);
        }
        result
    }
}
fn slug(name: &str) -> String {
    let ascii = deunicode::deunicode(name).to_lowercase();
    let value = ascii
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if value.is_empty() {
        "unidentified".into()
    } else {
        value.chars().take(200).collect()
    }
}
pub(crate) fn natural(left: &str, right: &str) -> std::cmp::Ordering {
    let left = left.to_lowercase();
    let right = right.to_lowercase();
    let mut a = left.chars().peekable();
    let mut b = right.chars().peekable();
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, _) => return std::cmp::Ordering::Less,
            (_, None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let x: String = std::iter::from_fn(|| a.next_if(char::is_ascii_digit)).collect();
                let y: String = std::iter::from_fn(|| b.next_if(char::is_ascii_digit)).collect();
                let x = x.trim_start_matches('0');
                let y = y.trim_start_matches('0');
                let order = x.len().cmp(&y.len()).then(x.cmp(y));
                if !order.is_eq() {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.cmp(&y);
                if !order.is_eq() {
                    return order;
                }
                a.next();
                b.next();
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_names_keep_accents_and_numeric_order_predictable() {
        assert_eq!(
            slug("Pokémon — Journey Together"),
            "pokemon-journey-together"
        );
        assert_eq!(slug("***"), "unidentified");
        assert!(natural("Card 2", "card 10").is_lt());
        assert!(natural("007", "7").is_eq());
    }
}
