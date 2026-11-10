use super::super::support::Harness;
use scanner_core::backend::CardProcessor;
use scanner_types::{
    error::Result,
    images::{CardPreparation, ImageCrop, ImageEditRequest, ProcessedImage},
    scan::{ScanRecord, ScanRequest},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use uuid::Uuid;
struct Processor(AtomicUsize);
#[async_trait::async_trait]
impl CardProcessor for Processor {
    async fn prepare(&self, record: &ScanRecord, sequence: u32) -> Result<ProcessedImage> {
        self.prepare_with_id(record, sequence, Uuid::new_v4()).await
    }
    async fn prepare_with_id(
        &self,
        record: &ScanRecord,
        sequence: u32,
        id: Uuid,
    ) -> Result<ProcessedImage> {
        self.0.fetch_add(1, Ordering::SeqCst);
        let root = scanner_files::StorageRoot::open(
            std::path::Path::new(&record.output_directory)
                .parent()
                .unwrap(),
        )?;
        if let Some(edit) = root.recover_preparation(record, id, sequence)? {
            return Ok(edit);
        }
        let original = record
            .images
            .iter()
            .find(|i| i.sequence == sequence)
            .unwrap();
        let bytes = root.read_image(record, original, 1024)?;
        let request = ImageEditRequest {
            id,
            job_id: record.id,
            sequence,
            crop: ImageCrop::default(),
            quarter_turns: 0,
            deskew_degrees: None,
            extensions: Default::default(),
        };
        let mut image = original.clone();
        image.relative_path = format!("edit-{id}.png");
        let plan = CardPreparation {
            crop: ImageCrop::default(),
            quarter_turns: 0,
            orientation_verified: false,
            warnings: vec!["Fixture orientation is uncertain.".into()],
            source_crop: None,
            deskew_degrees: None,
            orientation_basis: None,
            method: "test".into(),
            extensions: Default::default(),
        };
        let edit = ProcessedImage {
            request,
            created_at: chrono::Utc::now(),
            source_sha256: original.sha256.clone(),
            image,
            card_preparation: Some(plan),
            extensions: Default::default(),
        };
        root.write_edit(record, &edit, &bytes)?;
        Ok(edit)
    }
}
#[tokio::test]
async fn preparation_retries_keep_one_immutable_copy_and_repair_missing_publication() {
    let processor = Arc::new(Processor(AtomicUsize::new(0)));
    let mut h = Harness::with_components(processor.clone(), None);
    let job = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    h.next().await.finish.send(true).unwrap();
    let original = h.terminal(job.id).await;
    while h.engine.status().await.active_job_id.is_some() {
        tokio::task::yield_now().await;
    }
    let id = Uuid::new_v4();
    let (first, retry) = tokio::join!(
        h.engine.prepare_card(job.id, 1, id),
        h.engine.prepare_card(job.id, 1, id)
    );
    let first = first.unwrap();
    assert_eq!(first.image.sha256, retry.unwrap().image.sha256);
    assert_eq!(processor.0.load(Ordering::SeqCst), 1);
    assert_eq!(
        h.engine.prepare_card(job.id, 2, id).await.unwrap_err().kind,
        "conflict"
    );
    let record = h.engine.scan_status(job.id).unwrap();
    assert_eq!(record.processed_images.as_ref().unwrap().len(), 1);
    assert_eq!(record.images[0].sha256, original.images[0].sha256);
    h.engine
        .storage()
        .verify_image(&record, &record.images[0])
        .unwrap();
    let mut lost = record.clone();
    lost.processed_images = None;
    h.engine.database().save(&lost).unwrap();
    let recovered = h.engine.prepare_card(job.id, 1, id).await.unwrap();
    assert_eq!(recovered.image.sha256, first.image.sha256);
    let restored = h.engine.scan_status(job.id).unwrap();
    assert_eq!(restored.processed_images.as_ref().unwrap().len(), 1);
    assert_eq!(
        h.engine
            .storage()
            .read_manifest(&restored)
            .unwrap()
            .processed_images
            .unwrap()[0]
            .request
            .id,
        id
    );
    h.engine.shutdown().await;
    assert_eq!(
        h.engine
            .prepare_card(job.id, 1, Uuid::new_v4())
            .await
            .unwrap_err()
            .kind,
        "busy"
    );
}
