use super::LibraryCatalogue;
use scanner_types::{
    cards::CardLanguage,
    error::{Result, ScannerError},
    inventory::{InventoryCatalogueCard as Card, InventoryCatalogueSource as Source},
};
use std::collections::BTreeSet;
impl LibraryCatalogue {
    pub(super) async fn load_checklist(&self, source: &Source) -> Result<Vec<Card>> {
        match (source.provider.as_str(), source.language) {
            ("tcgdex", _) => Ok(self
                .pokemon(source.language)
                .set(&source.set_id)
                .await?
                .cards
                .into_iter()
                .map(|c| Card {
                    id: c.id,
                    name: c.name,
                    number: c.local_id,
                    language: source.language.as_str().into(),
                    thumbnail_key: c
                        .image
                        .and_then(|url| self.references.register(&format!("{url}/low.png"))),
                })
                .collect()),
            ("swu-db", CardLanguage::English) => Ok(self
                .star_wars
                .cards(&source.set_id)
                .await?
                .iter()
                .map(|c| Card {
                    id: format!("swu-db:{}-{}", c.set, c.number),
                    name: format!(
                        "{}{}",
                        c.name,
                        c.subtitle
                            .as_ref()
                            .map(|s| format!(" — {s}"))
                            .unwrap_or_default()
                    ),
                    number: c.number.clone(),
                    language: "en".into(),
                    thumbnail_key: c
                        .front_art
                        .as_deref()
                        .and_then(|url| self.references.register(url)),
                })
                .collect()),
            ("ygoprodeck", CardLanguage::English) => self.yugioh_checklist(&source.set_id).await,
            _ => Err(ScannerError::new(
                "unsupported",
                "No checklist for this catalogue and language.",
            )),
        }
    }
    async fn yugioh_checklist(&self, set: &str) -> Result<Vec<Card>> {
        if set.is_empty()
            || set.len() > 16
            || !set
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            return Err(ScannerError::new(
                "invalidParams",
                "Invalid Yu-Gi-Oh! set identifier.",
            ));
        }
        let catalogue = self.yugioh.cards().await?;
        let mut result = vec![];
        for card in catalogue.iter() {
            let codes: BTreeSet<_> = card
                .card_sets
                .as_deref()
                .unwrap_or_default()
                .iter()
                .filter(|p| english_code(&p.set_code, set))
                .map(|p| &p.set_code)
                .collect();
            for code in codes {
                if result.len() >= 5000 {
                    return Err(ScannerError::new(
                        "resourceLimit",
                        "Checklist exceeds 5000 cards.",
                    ));
                }
                result.push(Card {
                    id: format!("ygoprodeck:{}:{code}", card.id),
                    name: card.name.clone(),
                    number: code.clone(),
                    language: "en".into(),
                    thumbnail_key: card
                        .card_images
                        .as_deref()
                        .unwrap_or_default()
                        .first()
                        .and_then(|i| {
                            self.references.register(&format!(
                                "https://images.ygoprodeck.com/images/cards/{}.jpg",
                                i.id
                            ))
                        }),
                });
            }
        }
        Ok(result)
    }
    pub(super) async fn fetch_date(&self, source: &Source) -> Result<Option<String>> {
        match source.provider.as_str() {
            "tcgdex" => Ok(self
                .pokemon(source.language)
                .set(&source.set_id)
                .await?
                .release_date),
            "ygoprodeck" => self.yugioh.release_date(&source.set_id).await,
            "swu-db" => Ok(self
                .star_wars
                .sets()
                .await?
                .iter()
                .find(|s| s.set_id == source.set_id)
                .and_then(|s| s.release_date.clone())),
            _ => Ok(None),
        }
    }
}
fn english_code(code: &str, set: &str) -> bool {
    let Some((prefix, suffix)) = code.split_once('-') else {
        return false;
    };
    let number = suffix
        .strip_prefix("EN")
        .or_else(|| suffix.strip_prefix("AE"))
        .unwrap_or(suffix);
    prefix == set && !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checklists_keep_english_and_legacy_printings_only() {
        for code in ["LOB-EN001", "LOB-AE001", "LOB-001"] {
            assert!(english_code(code, "LOB"));
        }
        for code in ["LOB-FR001", "LOB-JP001", "LOB-EN", "LOB-EN00A", "MFC-EN001"] {
            assert!(!english_code(code, "LOB"));
        }
    }
}
