use crate::{
    cache::Cache,
    transport::{Https, JsonTransport},
    validation,
};
use scanner_types::{
    cards::CardLanguage,
    catalogue::{CardDetail, CatalogueCard, SetDetail},
    error::Result,
};
use serde::de::DeserializeOwned;
use std::{sync::Arc, time::Duration};

pub struct TcgDex {
    language: CardLanguage,
    transport: Arc<dyn JsonTransport>,
    catalogue: Cache<Vec<CatalogueCard>>,
    details: Cache<CardDetail>,
}
impl TcgDex {
    pub fn new(language: CardLanguage) -> Result<Self> {
        Ok(Self::with_transport(language, Arc::new(Https::new()?)))
    }
    pub fn with_transport(language: CardLanguage, transport: Arc<dyn JsonTransport>) -> Self {
        Self {
            language,
            transport,
            catalogue: Cache::new(1, Duration::from_secs(24 * 3600)),
            details: Cache::new(128, Duration::from_secs(15 * 60)),
        }
    }
    pub async fn catalogue(&self) -> Result<Arc<Vec<CatalogueCard>>> {
        self.catalogue
            .get("cards", || async {
                validation::catalogue(self.load("cards", 12 * 1024 * 1024).await?)
            })
            .await
    }
    pub async fn detail(&self, id: &str) -> Result<Arc<CardDetail>> {
        validation::id(id)?;
        self.details
            .get(id, || async {
                let detail = self.load(&format!("cards/{id}"), 256 * 1024).await?;
                validation::detail(&detail, id)?;
                Ok(detail)
            })
            .await
    }
    pub async fn set(&self, id: &str) -> Result<SetDetail> {
        validation::id(id)?;
        let set = self.load(&format!("sets/{id}"), 2 * 1024 * 1024).await?;
        validation::set(&set, id)?;
        Ok(set)
    }
    async fn load<T: DeserializeOwned>(&self, path: &str, limit: usize) -> Result<T> {
        let language = match self.language {
            CardLanguage::English => "en",
            CardLanguage::Korean => "ko",
        };
        let bytes = self
            .transport
            .get(&format!("{language}/{path}"), limit)
            .await?;
        if bytes.len() > limit {
            return Err(validation::invalid());
        }
        serde_json::from_slice(&bytes).map_err(|_| validation::invalid())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;
    struct Fixture {
        calls: Mutex<Vec<String>>,
        body: Vec<u8>,
    }
    #[async_trait]
    impl JsonTransport for Fixture {
        async fn get(&self, path: &str, _limit: usize) -> Result<Vec<u8>> {
            self.calls.lock().unwrap().push(path.into());
            Ok(self.body.clone())
        }
    }
    fn source(body: serde_json::Value) -> Arc<Fixture> {
        Arc::new(Fixture {
            calls: Mutex::new(vec![]),
            body: serde_json::to_vec(&body).unwrap(),
        })
    }
    #[tokio::test]
    async fn catalogue_filters_digital_cards_and_caches_language_specific_reads() {
        let fixture = source(serde_json::json!([
            {"id":"sv1-1","localId":"1","name":"뮤","image":"https://assets.tcgdex.net/ko/sv/sv1/1"},
            {"id":"A1-1","localId":"1","name":"Digital","image":"https://assets.tcgdex.net/ko/tcgp/A1/1"}
        ]));
        let client = TcgDex::with_transport(CardLanguage::Korean, fixture.clone());
        assert_eq!(client.catalogue().await.unwrap().len(), 1);
        assert_eq!(client.catalogue().await.unwrap()[0].id, "sv1-1");
        assert_eq!(*fixture.calls.lock().unwrap(), ["ko/cards"]);
    }
    #[tokio::test]
    async fn wrong_detail_identity_is_not_cached_and_paths_are_validated_first() {
        let fixture = source(
            serde_json::json!({"id":"other-1","localId":"1","name":"A","set":{"id":"other","name":"Other"}}),
        );
        let client = TcgDex::with_transport(CardLanguage::English, fixture.clone());
        assert!(client.detail("../escape").await.is_err());
        assert!(fixture.calls.lock().unwrap().is_empty());
        assert!(client.detail("sv1-1").await.is_err());
        assert!(client.detail("sv1-1").await.is_err());
        assert_eq!(fixture.calls.lock().unwrap().len(), 2);
    }
}
