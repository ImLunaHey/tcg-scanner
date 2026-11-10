use super::{super::support::Harness, condition::saved_pair};
use scanner_pricing::provider::PricingProvider;
use scanner_types::{
    error::Result,
    identification::IdentificationState,
    pricing::{CardMarketPrices, EuroSterlingRate},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
struct Provider {
    calls: AtomicUsize,
    gate: tokio::sync::Semaphore,
    started: tokio::sync::mpsc::UnboundedSender<()>,
}
impl Provider {
    async fn enter(&self) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.started.send(()).unwrap();
        self.gate.acquire().await.unwrap().forget();
    }
}
#[async_trait::async_trait]
impl PricingProvider for Provider {
    async fn prices(&self, id: &str) -> Result<CardMarketPrices> {
        self.enter().await;
        Ok(serde_json::from_value(serde_json::json!({"cardID":id,"fetchedAt":chrono::Utc::now(),"requiresPrintingReview":false,
            "variants":[{"finish":"holo","stamps":[],"euros":1.25,"updated":chrono::Utc::now()}]})).unwrap())
    }
    async fn sterling_rate(&self) -> Result<EuroSterlingRate> {
        self.enter().await;
        Ok(serde_json::from_value(serde_json::json!({"poundsPerEuro":0.9,"date":chrono::Utc::now().format("%Y-%m-%d").to_string(),"fetchedAt":chrono::Utc::now()})).unwrap())
    }
}
#[tokio::test]
async fn valuation_reads_do_not_wait_for_network_and_shared_fetches_persist() {
    let (h, mut record, mut identity, _) = saved_pair(Harness::new()).await;
    identity.state = IdentificationState::ReviewRequired;
    identity.evidence=Some(serde_json::from_value(serde_json::json!({"lines":[],"warnings":[],"orientation":0,"candidates":[
        {"id":"sv01-104","localID":"104","name":"Greavard","setID":"sv01","setName":"SV","evidence":["Collector number 104"],"availableFinishes":["holo"]}]})).unwrap());
    record.replace_identification(&identity).unwrap();
    h.engine.database().save(&record).unwrap();
    let (started, mut events) = tokio::sync::mpsc::unbounded_channel();
    let provider = Arc::new(Provider {
        calls: AtomicUsize::new(0),
        gate: tokio::sync::Semaphore::new(0),
        started,
    });
    h.engine.configure_pricing(provider.clone()).unwrap();
    let pending = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        h.engine.batch_valuation(record.id),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(pending.refreshing);
    assert!(pending.low_pence.is_none());
    assert_eq!(pending.total_count, 1);
    for _ in 0..2 {
        tokio::time::timeout(std::time::Duration::from_secs(3), events.recv())
            .await
            .unwrap()
            .unwrap();
    }
    for _ in 0..3 {
        assert!(
            h.engine
                .batch_valuation(record.id)
                .await
                .unwrap()
                .refreshing
        );
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    provider.gate.add_permits(2);
    let priced = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let snapshot = h.engine.batch_valuation(record.id).await.unwrap();
            if !snapshot.refreshing {
                break snapshot;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(priced.low_pence, Some(113));
    assert_eq!(priced.priced_count, 1);
    assert!(
        h.engine
            .database()
            .cached_value("card-variants-v2:sv01-104")
            .unwrap()
            .is_some()
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    let mut korean = record;
    korean.request.card_language = Some(scanner_types::cards::CardLanguage::Korean);
    h.engine.database().save(&korean).unwrap();
    let unsupported = h.engine.batch_valuation(korean.id).await.unwrap();
    assert!(unsupported.low_pence.is_none());
    assert!(!unsupported.refreshing);
    h.engine.shutdown().await;
}
