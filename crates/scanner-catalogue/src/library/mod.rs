//! Inventory checklists, asynchronous release dates and registered public previews.
mod dates;
mod image_network;
mod references;
use crate::{JsonTransport, TcgDex, cache::Cache, transport::Https};
pub use dates::ReleaseDates;
pub use image_network::ImageTransport;
use scanner_types::{
    cards::CardLanguage,
    error::{Result, ScannerError},
    inventory::{InventoryCatalogueCard, InventoryCatalogueSource},
};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
pub struct LibraryCatalogue {
    english: TcgDex,
    korean: TcgDex,
    cards: Cache<Vec<InventoryCatalogueCard>>,
    workers: Semaphore,
    dates: std::sync::Mutex<dates::State>,
    references: references::References,
}
impl LibraryCatalogue {
    pub fn new() -> Result<Arc<Self>> {
        Ok(Self::with_transports(
            Arc::new(Https::new()?),
            Arc::new(image_network::Https::new()?),
        ))
    }
    pub fn with_transports(
        json: Arc<dyn JsonTransport>,
        images: Arc<dyn ImageTransport>,
    ) -> Arc<Self> {
        Arc::new(Self {
            english: TcgDex::with_transport(CardLanguage::English, json.clone()),
            korean: TcgDex::with_transport(CardLanguage::Korean, json),
            cards: Cache::new(64, Duration::from_secs(86400)),
            workers: Semaphore::new(4),
            dates: Default::default(),
            references: references::References::new(images),
        })
    }
    pub async fn cards(
        &self,
        source: &InventoryCatalogueSource,
    ) -> Result<Arc<Vec<InventoryCatalogueCard>>> {
        let key = serde_json::to_string(source).map_err(unavailable)?;
        self.cards
            .get(&key, || async {
                let _permit = self.workers.try_acquire().map_err(|_| {
                    ScannerError::new("resourceLimit", "Catalogue is busy; retry shortly.")
                })?;
                if source.provider != "tcgdex" {
                    return Err(ScannerError::new(
                        "unsupported",
                        "This catalogue's native checklist adapter is not configured yet.",
                    ));
                }
                let set = self.pokemon(source.language).set(&source.set_id).await?;
                let mut cards: Vec<_> = set
                    .cards
                    .into_iter()
                    .map(|card| {
                        let thumbnail_key = card
                            .image
                            .and_then(|url| self.references.register(&format!("{url}/low.png")));
                        InventoryCatalogueCard {
                            id: card.id,
                            name: card.name,
                            number: card.local_id,
                            language: source.language.as_str().into(),
                            thumbnail_key,
                        }
                    })
                    .collect();
                cards.sort_by(|a, b| number_order(&a.number, &b.number).then(a.id.cmp(&b.id)));
                Ok(cards)
            })
            .await
    }
    fn pokemon(&self, language: CardLanguage) -> &TcgDex {
        match language {
            CardLanguage::English => &self.english,
            CardLanguage::Korean => &self.korean,
        }
    }
    pub async fn thumbnail(
        &self,
        key: &str,
    ) -> Result<scanner_types::image_access::ImageThumbnail> {
        self.references.thumbnail(key).await
    }
}
fn number_order(a: &str, b: &str) -> std::cmp::Ordering {
    match (a.parse::<u32>(), b.parse::<u32>()) {
        (Ok(a), Ok(b)) => a.cmp(&b),
        _ => a.cmp(b),
    }
}
fn unavailable(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("catalogueUnavailable", error.to_string())
}
