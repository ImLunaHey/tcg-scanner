//! Fixed-origin SWU-DB set and printing metadata; English only.
mod model;
use crate::{JsonTransport, cache::Cache, transport::Https, validation::invalid};
pub use model::{Card, Set};
use scanner_types::error::Result;
use std::{sync::Arc, time::Duration};
pub struct StarWars {
    transport: Arc<dyn JsonTransport>,
    documents: Option<Arc<crate::persistent::Documents>>,
    sets: Cache<Vec<Set>>,
    cards: Cache<Vec<Card>>,
}
impl StarWars {
    pub fn new() -> Result<Self> {
        Ok(Self::with_transport(Arc::new(Https::star_wars()?)))
    }
    pub fn with_transport(transport: Arc<dyn JsonTransport>) -> Self {
        Self {
            transport,
            documents: None,
            sets: Cache::new(1, Duration::from_secs(86400)),
            cards: Cache::new(32, Duration::from_secs(86400)),
        }
    }
    pub fn with_documents(mut self, documents: Arc<crate::persistent::Documents>) -> Self {
        self.documents = Some(documents);
        self
    }
    pub async fn sets(&self) -> Result<Arc<Vec<Set>>> {
        self.sets
            .get("sets", || async {
                crate::persistent::load(
                    self.transport.as_ref(),
                    self.documents.as_ref(),
                    "swu-db/sets",
                    "sets",
                    8 * 1024 * 1024,
                    Duration::from_secs(86400),
                    decode_sets,
                )
                .await
            })
            .await
    }
    pub async fn cards(&self, set: &str) -> Result<Arc<Vec<Card>>> {
        if !model::valid_id(set) {
            return Err(invalid());
        }
        self.cards
            .get(set, || async {
                let expected = set.to_owned();
                crate::persistent::load(
                    self.transport.as_ref(),
                    self.documents.as_ref(),
                    &format!("swu-db/cards/{set}"),
                    &format!("cards/{}", set.to_ascii_lowercase()),
                    8 * 1024 * 1024,
                    Duration::from_secs(86400),
                    move |bytes| decode(bytes, &expected),
                )
                .await
            })
            .await
    }
}
fn decode_sets(bytes: &[u8]) -> Result<Vec<Set>> {
    let sets: Vec<Set> = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    let unique: std::collections::HashSet<_> = sets.iter().map(|s| &s.set_id).collect();
    if sets.len() >= 500
        || unique.len() != sets.len()
        || sets.iter().any(|s| {
            s.set_id.is_empty()
                || s.set_id.len() > 16
                || !s
                    .set_id
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
                || s.full_name.is_empty()
                || s.full_name.len() > 128
                || s.number_cards == 0
                || s.number_cards > 5000
                || s.release_date.as_ref().is_some_and(|d| d.len() > 32)
        })
    {
        return Err(invalid());
    }
    Ok(sets)
}

fn decode(bytes: &[u8], set: &str) -> Result<Vec<Card>> {
    let result: model::Envelope = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if !model::valid_cards(&result.data, set) {
        return Err(invalid());
    }
    Ok(result.data)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrong_sets_and_duplicate_printing_numbers_are_rejected() {
        let card = serde_json::json!({"Set":"SOR","Number":"001","Name":"Card","Type":"Leader"});
        let one = serde_json::to_vec(&serde_json::json!({"data":[card.clone()]})).unwrap();
        assert!(decode(&one, "SOR").is_ok());
        assert!(decode(&one, "SHD").is_err());
        let duplicate =
            serde_json::to_vec(&serde_json::json!({"data":[card.clone(),card]})).unwrap();
        assert!(decode(&duplicate, "SOR").is_err());
        for id in ["../SOR", "sor", "SOR?", "", "A"] {
            assert!(!model::valid_id(id));
        }
    }
    #[test]
    fn auxiliary_short_set_codes_do_not_block_the_main_set_index() {
        let bytes=br#"[{"setId":"GG","fullName":"Gamegenic","numberCards":6},{"setId":"SOR","fullName":"Spark of Rebellion","numberCards":252}]"#;
        assert_eq!(decode_sets(bytes).unwrap().len(), 2);
        assert!(!model::valid_id("GG"));
    }
}
