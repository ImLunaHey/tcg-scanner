//! Fixed-origin SWU-DB set and printing metadata; English only.
mod model;
use crate::{JsonTransport, cache::Cache, transport::Https, validation::invalid};
pub use model::{Card, Set};
use scanner_types::error::Result;
use std::{sync::Arc, time::Duration};
pub struct StarWars {
    transport: Arc<dyn JsonTransport>,
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
            sets: Cache::new(1, Duration::from_secs(86400)),
            cards: Cache::new(32, Duration::from_secs(86400)),
        }
    }
    pub async fn sets(&self) -> Result<Arc<Vec<Set>>> {
        self.sets
            .get("sets", || async {
                let bytes = self.get("sets").await?;
                let sets: Vec<Set> = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
                let unique: std::collections::HashSet<_> = sets.iter().map(|s| &s.set_id).collect();
                if sets.len() >= 500
                    || unique.len() != sets.len()
                    || sets.iter().any(|s| {
                        !model::valid_id(&s.set_id)
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
            })
            .await
    }
    pub async fn cards(&self, set: &str) -> Result<Arc<Vec<Card>>> {
        if !model::valid_id(set) {
            return Err(invalid());
        }
        self.cards
            .get(set, || async {
                let bytes = self
                    .get(&format!("cards/{}", set.to_ascii_lowercase()))
                    .await?;
                let expected = set.to_owned();
                tokio::task::spawn_blocking(move || decode(&bytes, &expected))
                    .await
                    .map_err(|_| invalid())?
            })
            .await
    }
    async fn get(&self, path: &str) -> Result<Vec<u8>> {
        let bytes = self.transport.get(path, 8 * 1024 * 1024).await?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(invalid());
        }
        Ok(bytes)
    }
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
}
