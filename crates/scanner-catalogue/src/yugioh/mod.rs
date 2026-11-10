//! English YGOPRODeck metadata, fetched once per two-day in-process cache window.
mod model;
use crate::{JsonTransport, cache::Cache, transport::Https, validation::invalid};
pub use model::{Artwork, Card, Printing};
use scanner_types::error::Result;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
pub struct Yugioh {
    transport: Arc<dyn JsonTransport>,
    cards: Cache<Vec<Card>>,
    dates: Cache<BTreeMap<String, String>>,
}
impl Yugioh {
    pub fn new() -> Result<Self> {
        Ok(Self::with_transport(Arc::new(Https::yugioh()?)))
    }
    pub fn with_transport(transport: Arc<dyn JsonTransport>) -> Self {
        Self {
            transport,
            cards: Cache::new(1, Duration::from_secs(2 * 86400)),
            dates: Cache::new(1, Duration::from_secs(86400)),
        }
    }
    pub async fn cards(&self) -> Result<Arc<Vec<Card>>> {
        self.cards
            .get("cards", || async {
                let bytes = self.transport.get("cardinfo.php", 64 * 1024 * 1024).await?;
                tokio::task::spawn_blocking(move || decode(&bytes))
                    .await
                    .map_err(|_| invalid())?
            })
            .await
    }
    pub async fn release_date(&self, code: &str) -> Result<Option<String>> {
        let dates = self
            .dates
            .get("dates", || async {
                let bytes = self.transport.get("cardsets.php", 2 * 1024 * 1024).await?;
                if bytes.len() > 2 * 1024 * 1024 {
                    return Err(invalid());
                }
                let sets: Vec<model::SetDate> =
                    serde_json::from_slice(&bytes).map_err(|_| invalid())?;
                if sets.len() > 5000
                    || sets.iter().any(|s| {
                        s.set_code.is_empty()
                            || s.set_code.len() > 40
                            || s.tcg_date.as_ref().is_some_and(|d| d.len() > 32)
                    })
                {
                    return Err(invalid());
                }
                Ok(sets
                    .into_iter()
                    .filter_map(|s| Some((s.set_code, s.tcg_date?)))
                    .collect())
            })
            .await?;
        Ok(dates.get(code).cloned())
    }
}
fn decode(bytes: &[u8]) -> Result<Vec<Card>> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(invalid());
    }
    let cards: model::Envelope = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if !model::valid(&cards.data) {
        return Err(invalid());
    }
    Ok(cards.data)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_ids_and_invalid_artwork_are_rejected() {
        let card = serde_json::json!({"id":12345678,"name":"A card","type":"Normal Monster","card_images":[{"id":12345678}]});
        let encode = |cards: serde_json::Value| {
            serde_json::to_vec(&serde_json::json!({"data":cards})).unwrap()
        };
        assert!(decode(&encode(serde_json::json!([card.clone()]))).is_ok());
        assert!(decode(&encode(serde_json::json!([card.clone(), card]))).is_err());
        let card = serde_json::json!({"id":1,"name":"A card","type":"Spell Card","card_images":[{"id":0}]});
        assert!(decode(&encode(serde_json::json!([card]))).is_err());
    }
}
