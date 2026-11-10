use super::super::support::Harness;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use scanner_types::{
    Extensions,
    image_access::ThumbnailFormat,
    images::{ImageCrop, ImageEditRequest},
    scan::ScanRequest,
};
use uuid::Uuid;

fn edit(job_id: Uuid) -> ImageEditRequest {
    ImageEditRequest {
        id: Uuid::new_v4(),
        job_id,
        sequence: 1,
        crop: ImageCrop::default(),
        quarter_turns: 1,
        deskew_degrees: None,
        extensions: Extensions::new(),
    }
}
async fn completed(h: &mut Harness) -> scanner_types::scan::ScanRecord {
    let job = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    h.next().await.finish.send(true).unwrap();
    let record = h.terminal(job.id).await;
    while h.engine.status().await.active_job_id.is_some() {
        tokio::task::yield_now().await;
    }
    record
}
#[tokio::test]
async fn immutable_edits_are_idempotent_and_recover_after_interrupted_publication() {
    let mut h = Harness::new();
    let record = completed(&mut h).await;
    let original = h
        .engine
        .storage()
        .read_image(&record, &record.images[0], 1024)
        .unwrap();
    let request = edit(record.id);
    let (first, retry) = tokio::join!(
        h.engine.edit_image(request.clone()),
        h.engine.edit_image(request.clone())
    );
    let first = first.unwrap();
    assert_eq!(first.image.sha256, retry.unwrap().image.sha256);
    assert_eq!((first.image.width, first.image.height), (9, 6));
    assert_eq!(
        h.engine
            .scan_status(record.id)
            .unwrap()
            .processed_images
            .unwrap()
            .len(),
        1
    );
    let mut conflict = request.clone();
    conflict.quarter_turns = 2;
    assert_eq!(
        h.engine.edit_image(conflict).await.unwrap_err().kind,
        "conflict"
    );
    // Simulate files surviving while the ledger commit did not.
    h.engine.database().save(&record).unwrap();
    let recovered = h.engine.edit_image(request.clone()).await.unwrap();
    assert_eq!(recovered.created_at, first.created_at);
    assert_eq!(recovered.image.sha256, first.image.sha256);
    assert_eq!(
        h.engine
            .storage()
            .read_image(&record, &record.images[0], 1024)
            .unwrap(),
        original
    );
    let saved: scanner_types::scan::ScanRecord = serde_json::from_slice(
        &std::fs::read(std::path::Path::new(&record.output_directory).join("manifest.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(saved.processed_images.unwrap()[0].request.id, request.id);
    h.engine.shutdown().await;
    assert_eq!(
        h.engine.edit_image(edit(record.id)).await.unwrap_err().kind,
        "busy"
    );
}
#[tokio::test]
async fn chunks_thumbnails_and_edit_selection_use_recorded_images() {
    let mut h = Harness::new();
    let record = completed(&mut h).await;
    let edited = h.engine.edit_image(edit(record.id)).await.unwrap();
    let metadata = h
        .engine
        .image_metadata(record.id, 1, Some(edited.request.id))
        .await
        .unwrap();
    assert_eq!(metadata.image.sha256, edited.image.sha256);
    let chunk = h
        .engine
        .read_image(record.id, 1, None, 0, 262144)
        .await
        .unwrap();
    assert_eq!(chunk.chunk_sha256, record.images[0].sha256);
    assert!(chunk.eof);
    assert_eq!(chunk.next_offset, record.images[0].bytes);
    let end = h
        .engine
        .read_image(record.id, 1, None, chunk.next_offset, 1)
        .await
        .unwrap();
    assert!(end.eof && end.base64.is_empty());
    assert!(
        h.engine
            .read_image(record.id, 1, None, chunk.next_offset + 1, 1)
            .await
            .is_err()
    );
    assert!(
        h.engine
            .image_metadata(record.id, 2, Some(edited.request.id))
            .await
            .is_err()
    );
    let thumbnail = h
        .engine
        .thumbnail(record.id, 1, Some(edited.request.id), ThumbnailFormat::Jpeg)
        .await
        .unwrap();
    assert_eq!(thumbnail.media_type, "image/jpeg");
    let decoded =
        scanner_images::decode::decode(&STANDARD.decode(thumbnail.base64).unwrap()).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (9, 6));
    let image_path =
        std::path::Path::new(&record.output_directory).join(&record.images[0].relative_path);
    let mut bytes = std::fs::read(&image_path).unwrap();
    bytes[0] ^= 1;
    std::fs::write(image_path, bytes).unwrap();
    assert!(
        h.engine
            .thumbnail(record.id, 1, None, ThumbnailFormat::Png)
            .await
            .is_err()
    );
    assert!(h.engine.edit_image(edit(record.id)).await.is_err());
    h.engine.shutdown().await;
}
#[tokio::test]
async fn active_scans_and_excess_edits_are_refused_without_overwriting_files() {
    let mut h = Harness::new();
    let job = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    let capture = h.next().await;
    assert_eq!(
        h.engine.edit_image(edit(job.id)).await.unwrap_err().kind,
        "busy"
    );
    capture.finish.send(true).unwrap();
    h.terminal(job.id).await;
    while h.engine.status().await.active_job_id.is_some() {
        tokio::task::yield_now().await;
    }
    for _ in 0..8 {
        h.engine.edit_image(edit(job.id)).await.unwrap();
    }
    assert_eq!(
        h.engine.edit_image(edit(job.id)).await.unwrap_err().kind,
        "resourceLimit"
    );
    h.engine.shutdown().await;
}
