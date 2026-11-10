use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use scanner_catalogue::{
    JsonTransport,
    library::{ImageTransport, LibraryCatalogue},
};
use scanner_types::{cards::CardLanguage, error::Result, inventory::InventoryCatalogueSource};
use std::{
    io::Cursor,
    sync::{Arc, Mutex},
};
#[derive(Default)]
struct Fixture {
    calls: Mutex<Vec<String>>,
}
#[async_trait]
impl JsonTransport for Fixture {
    async fn get(&self, path: &str, _: usize) -> Result<Vec<u8>> {
        self.calls.lock().unwrap().push(path.into());
        tokio::task::yield_now().await;
        Ok(serde_json::to_vec(&serde_json::json!({"id":"sv1", "releaseDate":"2023-03-31", "cards":[
            {"id":"sv1-10","localId":"10","name":"Ten","image":"https://assets.tcgdex.net/en/sv/sv1/10"},
            {"id":"sv1-2","localId":"2","name":"Two","image":"https://localhost/private"}
        ]})).unwrap())
    }
}
#[async_trait]
impl ImageTransport for Fixture {
    async fn get(&self, url: &str, limit: usize) -> Result<Vec<u8>> {
        assert_eq!(limit, 8 * 1024 * 1024);
        self.calls.lock().unwrap().push(url.into());
        let image = image::RgbImage::from_pixel(40, 60, image::Rgb([230, 220, 100]));
        let mut out = Cursor::new(vec![]);
        image.write_to(&mut out, image::ImageFormat::Png).unwrap();
        Ok(out.into_inner())
    }
}
fn source() -> InventoryCatalogueSource {
    InventoryCatalogueSource {
        provider: "tcgdex".into(),
        set_id: "sv1".into(),
        language: CardLanguage::English,
    }
}
#[tokio::test]
async fn checklist_and_dates_share_fetches_and_only_registered_previews_are_loaded() {
    let json = Arc::new(Fixture::default());
    let images = Arc::new(Fixture::default());
    let catalogue = LibraryCatalogue::with_transports(json.clone(), images.clone());
    let source = source();
    assert!(
        catalogue
            .release_dates(std::slice::from_ref(&source))
            .pending
    );
    let (a, b) = tokio::join!(catalogue.cards(&source), catalogue.cards(&source));
    let cards = a.unwrap();
    assert_eq!(b.unwrap().len(), 2);
    assert_eq!(cards[0].number, "2");
    assert!(cards[0].thumbnail_key.is_none());
    let key = cards[1].thumbnail_key.as_deref().unwrap();
    let (a, b) = tokio::join!(catalogue.thumbnail(key), catalogue.thumbnail(key));
    let thumbnail = a.unwrap();
    assert_eq!(b.unwrap().sha256, thumbnail.sha256);
    assert_eq!((thumbnail.width, thumbnail.height), (40, 60));
    assert_eq!(thumbnail.media_type, "image/jpeg");
    let bytes = STANDARD.decode(thumbnail.base64).unwrap();
    assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 40);
    assert_eq!(images.calls.lock().unwrap().len(), 1);
    assert!(catalogue.thumbnail(&"0".repeat(64)).await.is_err());
    assert!(catalogue.thumbnail("https://localhost").await.is_err());
    assert_eq!(images.calls.lock().unwrap().len(), 1);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let dates = catalogue.release_dates(std::slice::from_ref(&source));
            if !dates.pending {
                assert_eq!(
                    dates.dates.get(&source).map(String::as_str),
                    Some("2023-03-31")
                );
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(*json.calls.lock().unwrap(), ["en/sets/sv1"]);
    let mut korean = source.clone();
    korean.language = CardLanguage::Korean;
    assert_eq!(catalogue.cards(&korean).await.unwrap()[0].language, "ko");
    assert_eq!(json.calls.lock().unwrap().len(), 2);
}
