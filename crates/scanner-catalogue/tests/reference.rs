use scanner_catalogue::{
    JsonTransport, library::ImageTransport, persistent::Documents, reference::Artwork,
};
use scanner_types::error::Result;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
struct Fixture {
    calls: Mutex<Vec<String>>,
    image_url: String,
    bytes: Vec<u8>,
}
#[async_trait::async_trait]
impl JsonTransport for Fixture {
    async fn get(&self, path: &str, _: usize) -> Result<Vec<u8>> {
        self.calls.lock().unwrap().push(path.into());
        Ok(serde_json::to_vec(
            &serde_json::json!({"id":"sv01-104","localId":"104","name":"Greavard",
            "set":{"id":"sv01","name":"Scarlet & Violet"},"image":self.image_url}),
        )
        .unwrap())
    }
}
#[async_trait::async_trait]
impl ImageTransport for Fixture {
    async fn get(&self, url: &str, limit: usize) -> Result<Vec<u8>> {
        assert_eq!(limit, 8 * 1024 * 1024);
        self.calls.lock().unwrap().push(url.into());
        Ok(self.bytes.clone())
    }
}
fn fixture(url: &str, valid: bool) -> Arc<Fixture> {
    let image = image::RgbImage::from_pixel(40, 60, image::Rgb([30, 40, 80]));
    let mut out = std::io::Cursor::new(vec![]);
    image.write_to(&mut out, image::ImageFormat::Png).unwrap();
    Arc::new(Fixture {
        calls: Default::default(),
        image_url: url.into(),
        bytes: if valid {
            out.into_inner()
        } else {
            b"corrupt".to_vec()
        },
    })
}
#[tokio::test]
async fn public_artwork_is_deduplicated_verified_and_persisted_without_scans() {
    let directory = tempfile::tempdir().unwrap();
    let documents = Documents::open(
        &directory
            .path()
            .canonicalize()
            .unwrap()
            .join("private/public.sqlite"),
    )
    .unwrap();
    let source = fixture("https://assets.tcgdex.net/en/sv/sv01/104", true);
    let artwork =
        Artwork::with_transports(source.clone(), source.clone()).with_documents(documents.clone());
    let (a, b) = tokio::join!(artwork.for_card("sv01-104"), artwork.for_card("sv01-104"));
    let a = a.unwrap();
    let b = b.unwrap();
    let a = a.as_ref().as_ref().unwrap();
    let b = b.as_ref().as_ref().unwrap();
    assert_eq!(a.sha256, b.sha256);
    assert_eq!(a.sha256, format!("{:x}", Sha256::digest(&source.bytes)));
    assert_eq!(source.calls.lock().unwrap().len(), 2);
    let restored =
        Artwork::with_transports(source.clone(), source.clone()).with_documents(documents.clone());
    assert_eq!(
        restored
            .for_card("sv01-104")
            .await
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap()
            .bytes,
        a.bytes
    );
    assert_eq!(source.calls.lock().unwrap().len(), 2);
    let key = format!("reference-original/{:x}", Sha256::digest(a.url.as_bytes()));
    documents.put(&key, b"invalid PNG".to_vec()).await.unwrap();
    let rebuilt =
        Artwork::with_transports(source.clone(), source.clone()).with_documents(documents);
    assert!(rebuilt.for_card("sv01-104").await.unwrap().is_some());
    assert_eq!(source.calls.lock().unwrap().len(), 3);
}
#[tokio::test]
async fn hostile_urls_and_corrupt_downloads_are_rejected_before_comparison() {
    let source = fixture("https://localhost/private", true);
    let artwork = Artwork::with_transports(source.clone(), source.clone());
    assert!(artwork.for_card("sv01-104").await.is_err());
    assert_eq!(source.calls.lock().unwrap().len(), 1);
    let source = fixture("https://assets.tcgdex.net/en/sv/sv01/104", false);
    let artwork = Artwork::with_transports(source.clone(), source.clone());
    assert!(artwork.for_card("sv01-104").await.is_err());
    assert_eq!(source.calls.lock().unwrap().len(), 2);
}
