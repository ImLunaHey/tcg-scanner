use super::super::{super::support::Harness, condition::saved_pair};
use scanner_staging::{
    Staging,
    contracts::{BatchReply, CardInput, CardReply, Import, ImportReply, Photo},
};
use scanner_types::{
    error::{Result, ScannerError},
    scan::ScanRecord,
};
use sha2::{Digest, Sha256};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use uuid::Uuid;

pub async fn prepared() -> (Harness, ScanRecord, Arc<Store>) {
    let (h, mut record, mut identity, _) = saved_pair(Harness::new()).await;
    let image = scanner_images::encode_png(image::RgbaImage::from_pixel(
        600,
        900,
        image::Rgba([20, 40, 70, 255]),
    ))
    .unwrap();
    for mut edit in record.processed_images.clone().unwrap() {
        edit.request.id = Uuid::new_v4();
        edit.image.relative_path = format!("edit-{}.png", edit.request.id);
        edit.image.width = image.width;
        edit.image.height = image.height;
        edit.image.sha256 = image.sha256.clone();
        edit.image.bytes = image.bytes.len() as u64;
        h.engine
            .storage()
            .write_edit(&record, &edit, &image.bytes)
            .unwrap();
        if edit.request.sequence == 2 {
            identity.source_sha256 = image.sha256.clone();
            identity.source_edit_id = Some(edit.request.id);
        }
        record.processed_images.as_mut().unwrap().push(edit);
    }
    record.replace_identification(&identity).unwrap();
    h.engine.database().save(&record).unwrap();
    h.engine.storage().save_manifest(&record).unwrap();
    h.engine.configure_pricing(Arc::new(NoPrices)).unwrap();
    let store = Arc::new(Store::new());
    h.engine.configure_staging(store.clone()).unwrap();
    (h, record, store)
}
struct NoPrices;
#[async_trait::async_trait]
impl scanner_pricing::provider::PricingProvider for NoPrices {
    async fn prices(&self, _: &str) -> Result<scanner_types::pricing::CardMarketPrices> {
        Err(ScannerError::new("fixture", "Offline"))
    }
    async fn sterling_rate(&self) -> Result<scanner_types::pricing::EuroSterlingRate> {
        Err(ScannerError::new("fixture", "Offline"))
    }
}
pub struct Store {
    pub remote: Mutex<Option<CardReply>>,
    pub imports: AtomicUsize,
    pub updates: AtomicUsize,
    pub uploads: AtomicUsize,
    pub lose_upload: AtomicBool,
    pub gate: tokio::sync::Semaphore,
    pub entered: tokio::sync::Notify,
    batch: Uuid,
}
impl Store {
    pub fn new() -> Self {
        Self {
            remote: Mutex::new(None),
            imports: AtomicUsize::new(0),
            updates: AtomicUsize::new(0),
            uploads: AtomicUsize::new(0),
            lose_upload: AtomicBool::new(false),
            gate: tokio::sync::Semaphore::new(100),
            entered: tokio::sync::Notify::new(),
            batch: Uuid::new_v4(),
        }
    }
}
#[async_trait::async_trait]
impl Staging for Store {
    fn origin(&self) -> &str {
        "https://store.example"
    }
    fn scanner_instance_id(&self) -> &str {
        "fixture-scanner"
    }
    async fn import(&self, value: &Import) -> Result<ImportReply> {
        self.imports.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        self.gate.acquire().await.unwrap().forget();
        let mut remote = self.remote.lock().unwrap();
        let input = &value.cards[0];
        let card = remote
            .get_or_insert_with(|| CardReply {
                id: Uuid::new_v4(),
                scan_batch_id: self.batch,
                card_number: input.card_number.clone(),
                revision: 1,
                status: "pending".into(),
                front: None,
                back: None,
                review: Some(input.review.clone()),
                confirmed: None,
                notes: Some(String::new()),
                game: input.game.clone(),
                language: input.language.clone(),
                game_source: input.game_source.clone(),
                language_source: input.language_source.clone(),
            })
            .clone();
        Ok(ImportReply {
            batch: BatchReply {
                id: self.batch,
                scanner_instance_id: value.scanner_instance_id.clone(),
                source_batch_id: value.source_batch_id,
                batch_number: value.batch_number.clone(),
                review_url: "/review/fixture".into(),
            },
            cards: vec![card],
        })
    }
    async fn update(&self, card: &CardReply, input: &CardInput) -> Result<CardReply> {
        self.updates.fetch_add(1, Ordering::SeqCst);
        let mut remote = self.remote.lock().unwrap();
        let remote = remote.as_mut().unwrap();
        assert_eq!(remote.revision, card.revision);
        remote.revision += 1;
        remote.review = Some(input.review.clone());
        remote.game = input.game.clone();
        remote.language = input.language.clone();
        remote.game_source = input.game_source.clone();
        remote.language_source = input.language_source.clone();
        Ok(remote.clone())
    }
    async fn upload(&self, card: &CardReply, front: Vec<u8>, back: Vec<u8>) -> Result<CardReply> {
        self.uploads.fetch_add(1, Ordering::SeqCst);
        let photo = |bytes: Vec<u8>| Photo {
            id: Uuid::new_v4(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            size_bytes: bytes.len() as u64,
        };
        let mut remote = self.remote.lock().unwrap();
        let remote = remote.as_mut().unwrap();
        assert_eq!(remote.revision, card.revision);
        remote.revision += 1;
        remote.front = Some(photo(front));
        remote.back = Some(photo(back));
        if self.lose_upload.swap(false, Ordering::SeqCst) {
            return Err(ScannerError::new("storeUnavailable", "Lost response"));
        }
        Ok(remote.clone())
    }
}
pub async fn settled(h: &Harness, id: Uuid) -> scanner_types::staging::TransferStatus {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let status = h.engine.store_transfer_status(id).await.unwrap();
            if status.state != scanner_types::staging::TransferState::Sending {
                return status;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}
