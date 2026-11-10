//! Inventory checklists, asynchronous release dates and registered public previews.
mod checklists;
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
    yugioh: crate::yugioh::Yugioh,
    star_wars: crate::star_wars::StarWars,
    cards: Cache<Vec<InventoryCatalogueCard>>,
    workers: Semaphore,
    dates: std::sync::Mutex<dates::State>,
    references: references::References,
}
impl LibraryCatalogue {
    pub fn new() -> Result<Arc<Self>> {
        Ok(Self::with_all_transports(
            Arc::new(Https::new()?),
            Arc::new(Https::yugioh()?),
            Arc::new(Https::star_wars()?),
            Arc::new(image_network::Https::new()?),
        ))
    }
    pub fn with_transports(
        json: Arc<dyn JsonTransport>,
        images: Arc<dyn ImageTransport>,
    ) -> Arc<Self> {
        Self::with_all_transports(json.clone(), json.clone(), json, images)
    }
    pub fn with_all_transports(
        pokemon: Arc<dyn JsonTransport>,
        yugioh: Arc<dyn JsonTransport>,
        star_wars: Arc<dyn JsonTransport>,
        images: Arc<dyn ImageTransport>,
    ) -> Arc<Self> {
        Self::from_clients(
            TcgDex::with_transport(CardLanguage::English, pokemon.clone()),
            TcgDex::with_transport(CardLanguage::Korean, pokemon),
            crate::yugioh::Yugioh::with_transport(yugioh),
            crate::star_wars::StarWars::with_transport(star_wars),
            references::References::new(images),
        )
    }
    pub fn cached(documents: Arc<crate::persistent::Documents>) -> Result<Arc<Self>> {
        Ok(Self::from_clients(
            TcgDex::new(CardLanguage::English)?.with_documents(documents.clone()),
            TcgDex::new(CardLanguage::Korean)?.with_documents(documents.clone()),
            crate::yugioh::Yugioh::new()?.with_documents(documents.clone()),
            crate::star_wars::StarWars::new()?.with_documents(documents.clone()),
            references::References::new(Arc::new(image_network::Https::new()?))
                .with_documents(documents),
        ))
    }
    fn from_clients(
        english: TcgDex,
        korean: TcgDex,
        yugioh: crate::yugioh::Yugioh,
        star_wars: crate::star_wars::StarWars,
        references: references::References,
    ) -> Arc<Self> {
        Arc::new(Self {
            english,
            korean,
            yugioh,
            star_wars,
            references,
            cards: Cache::new(64, Duration::from_secs(86400)),
            workers: Semaphore::new(4),
            dates: Default::default(),
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
                let mut cards = self.load_checklist(source).await?;
                let unique: std::collections::HashSet<_> = cards.iter().map(|c| &c.id).collect();
                if cards.is_empty() || cards.len() > 5000 || unique.len() != cards.len() {
                    return Err(ScannerError::new(
                        "catalogueUnavailable",
                        "No usable checklist for this set yet.",
                    ));
                }
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
