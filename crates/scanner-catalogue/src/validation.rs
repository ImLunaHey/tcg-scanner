use scanner_types::{
    catalogue::{CardDetail, CatalogueCard, SetDetail},
    error::{Result, ScannerError},
};
use std::collections::HashSet;
pub(crate) fn invalid() -> ScannerError {
    ScannerError::new("catalogue", "Invalid or oversized catalogue metadata.")
}
pub(crate) fn id(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 100
        || matches!(value, "." | "..")
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+.-".contains(&b))
    {
        Err(invalid())
    } else {
        Ok(())
    }
}
pub(crate) fn catalogue(cards: Vec<CatalogueCard>) -> Result<Vec<CatalogueCard>> {
    if cards.len() > 100_000 {
        return Err(invalid());
    }
    Ok(cards
        .into_iter()
        .filter(|c| {
            id(&c.id).is_ok()
                && !c.name.is_empty()
                && c.name.len() <= 128
                && c.local_id.len() <= 32
                && !c.image.as_deref().is_some_and(|url| url.contains("/tcgp/"))
        })
        .collect())
}
pub(crate) fn detail(card: &CardDetail, expected: &str) -> Result<()> {
    if card.id != expected
        || card.local_id.len() > 32
        || card.name.is_empty()
        || card.name.len() > 128
        || id(&card.set.id).is_err()
        || card.set.name.len() > 256
        || card.category.as_ref().is_some_and(|s| s.len() > 64)
        || card.rarity.as_ref().is_some_and(|s| s.len() > 128)
        || card
            .types
            .as_ref()
            .is_some_and(|types| types.len() > 8 || types.iter().any(|t| t.len() > 64))
        || card.variants_detailed.as_ref().is_some_and(|variants| {
            variants.len() > 100
                || variants.iter().any(|v| {
                    v.variant_id.as_ref().is_some_and(|s| s.len() > 128)
                        || v.variant_type.len() > 64
                        || v.stamp.as_ref().is_some_and(|stamps| {
                            stamps.len() > 8 || stamps.iter().any(|s| s.len() > 80)
                        })
                })
        })
    {
        Err(invalid())
    } else {
        Ok(())
    }
}
pub(crate) fn set(set: &SetDetail, expected: &str) -> Result<()> {
    let unique: HashSet<_> = set.cards.iter().map(|c| &c.id).collect();
    if set.id != expected
        || set.cards.is_empty()
        || set.cards.len() > 5000
        || unique.len() != set.cards.len()
        || set.release_date.as_ref().is_some_and(|s| s.len() > 32)
        || set.cards.iter().any(|c| {
            !c.id.starts_with(&format!("{expected}-"))
                || id(&c.id).is_err()
                || c.name.is_empty()
                || c.name.len() > 256
                || c.local_id.len() > 32
        })
    {
        Err(invalid())
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identifiers_cannot_change_request_paths() {
        for bad in ["../cards", "..", ".", "sv/1", "id?x=y", "id%2fother", ""] {
            assert!(id(bad).is_err(), "{bad}");
        }
        for good in ["sv08.5-001", "S4a+-001", "bwp-BW92"] {
            assert!(id(good).is_ok());
        }
    }
    #[test]
    fn sets_require_unique_ids_in_the_requested_set() {
        let mut value: SetDetail = serde_json::from_value(
            serde_json::json!({"id":"sv1","cards":[{"id":"sv1-1","name":"A","localId":"1"}]}),
        )
        .unwrap();
        assert!(set(&value, "sv1").is_ok());
        value.cards.push(value.cards[0].clone());
        assert!(set(&value, "sv1").is_err());
        value.cards.pop();
        value.cards[0].id = "sv2-1".into();
        assert!(set(&value, "sv1").is_err());
    }
}
