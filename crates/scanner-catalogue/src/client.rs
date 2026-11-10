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
    sets: Cache<SetDetail>,
    documents: Option<Arc<crate::persistent::Documents>>,
}
impl TcgDex {
    pub fn new(language: CardLanguage) -> Result<Self> {
        Ok(Self::with_transport(language, Arc::new(Https::new()?)))
    }
    pub fn with_transport(language: CardLanguage, transport: Arc<dyn JsonTransport>) -> Self {
        Self {
            language,
            transport,
            documents: None,
            catalogue: Cache::new(1, Duration::from_secs(24 * 3600)),
            details: Cache::new(128, Duration::from_secs(15 * 60)),
            sets: Cache::new(64, Duration::from_secs(24 * 3600)),
        }
    }
    pub fn with_documents(mut self, documents: Arc<crate::persistent::Documents>) -> Self {
        self.documents = Some(documents);
        self
    }
    pub async fn catalogue(&self) -> Result<Arc<Vec<CatalogueCard>>> {
        self.catalogue
            .get("cards", || async {
                self.load("cards", 12 * 1024 * 1024, validation::catalogue)
                    .await
            })
            .await
    }
    pub async fn detail(&self, id: &str) -> Result<Arc<CardDetail>> {
        validation::id(id)?;
        self.details
            .get(id, || async {
                let expected = id.to_owned();
                self.load(&format!("cards/{id}"), 256 * 1024, move |detail| {
                    validation::detail(&detail, &expected)?;
                    Ok(detail)
                })
                .await
            })
            .await
    }
    pub async fn set(&self, id: &str) -> Result<SetDetail> {
        validation::id(id)?;
        let set = self
            .sets
            .get(id, || async {
                let expected = id.to_owned();
                self.load(&format!("sets/{id}"), 2 * 1024 * 1024, move |set| {
                    validation::set(&set, &expected)?;
                    Ok(set)
                })
                .await
            })
            .await?;
        Ok(set.as_ref().clone())
    }
    async fn load<T, F>(&self, path: &str, limit: usize, validate: F) -> Result<T>
    where
        T: DeserializeOwned + Send + 'static,
        F: Fn(T) -> Result<T> + Send + Sync + 'static,
    {
        let path = format!("{}/{path}", self.language.as_str());
        let ttl = if path.contains("/cards/") { 900 } else { 86400 };
        crate::persistent::load(
            self.transport.as_ref(),
            self.documents.as_ref(),
            &format!("tcgdex/{path}"),
            &path,
            limit,
            Duration::from_secs(ttl),
            move |bytes| {
                validate(serde_json::from_slice(bytes).map_err(|_| validation::invalid())?)
            },
        )
        .await
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
