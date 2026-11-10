use async_trait::async_trait;
use scanner_catalogue::{JsonTransport, TcgDex, persistent::Documents};
use scanner_types::{cards::CardLanguage, error::Result};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
struct Fixture {
    calls: AtomicUsize,
}
#[async_trait]
impl JsonTransport for Fixture {
    async fn get(&self, path: &str, _: usize) -> Result<Vec<u8>> {
        assert_eq!(path, "en/sets/sv1");
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(br#"{"id":"sv1","cards":[{"id":"sv1-1","name":"One","localId":"1"}]}"#.to_vec())
    }
}
#[tokio::test]
async fn reopening_reuses_validated_documents_and_invalid_identity_is_refetched() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .canonicalize()
        .unwrap()
        .join("public/data.sqlite");
    let fixture = Arc::new(Fixture {
        calls: AtomicUsize::new(0),
    });
    {
        let client = TcgDex::with_transport(CardLanguage::English, fixture.clone())
            .with_documents(Documents::open(&path).unwrap());
        assert_eq!(client.set("sv1").await.unwrap().cards.len(), 1);
    }
    let documents = Documents::open(&path).unwrap();
    let client = TcgDex::with_transport(CardLanguage::English, fixture.clone())
        .with_documents(documents.clone());
    assert_eq!(client.set("sv1").await.unwrap().cards[0].id, "sv1-1");
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
    drop(client);
    // Valid JSON and checksum are insufficient: a cached response must also
    // describe the requested set before the catalogue accepts it.
    documents
        .put(
            "tcgdex/en/sets/sv1",
            br#"{"id":"sv2","cards":[{"id":"sv2-1","name":"Wrong","localId":"1"}]}"#.to_vec(),
        )
        .await
        .unwrap();
    let client =
        TcgDex::with_transport(CardLanguage::English, fixture.clone()).with_documents(documents);
    assert_eq!(client.set("sv1").await.unwrap().cards[0].id, "sv1-1");
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 2);
}
