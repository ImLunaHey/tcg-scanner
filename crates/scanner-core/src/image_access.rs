use crate::Engine;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use scanner_types::{
    error::{Result, ScannerError},
    image_access::{ImageChunk, ImageMetadata, ImageThumbnail, ThumbnailFormat},
    images::OriginalImage,
    scan::ScanRecord,
};
use std::sync::Arc;
use uuid::Uuid;

impl Engine {
    pub async fn image_metadata(
        self: &Arc<Self>,
        job: Uuid,
        sequence: u32,
        edit: Option<Uuid>,
    ) -> Result<ImageMetadata> {
        let engine = self.clone();
        tokio::task::spawn_blocking(move || {
            let record = engine.scan_status(job)?;
            Ok(metadata(job, selected(&record, sequence, edit)?))
        })
        .await
        .map_err(worker)?
    }
    pub async fn read_image(
        self: &Arc<Self>,
        job: Uuid,
        sequence: u32,
        edit: Option<Uuid>,
        offset: u64,
        length: usize,
    ) -> Result<ImageChunk> {
        if offset > 512 * 1024 * 1024 || !(1..=262_144).contains(&length) {
            return Err(ScannerError::new(
                "invalidParams",
                "Invalid image offset or chunk length.",
            ));
        }
        let engine = self.clone();
        tokio::task::spawn_blocking(move || {
            let record = engine.scan_status(job)?;
            let image = selected(&record, sequence, edit)?;
            let chunk = engine.root.read_chunk(&record, image, offset, length)?;
            Ok(ImageChunk {
                schema_version: 1,
                metadata: metadata(job, image),
                offset,
                next_offset: offset + chunk.bytes.len() as u64,
                eof: chunk.eof,
                base64: STANDARD.encode(chunk.bytes),
                chunk_sha256: chunk.sha256,
            })
        })
        .await
        .map_err(worker)?
    }
    pub async fn thumbnail(
        self: &Arc<Self>,
        job: Uuid,
        sequence: u32,
        edit: Option<Uuid>,
        format: ThumbnailFormat,
    ) -> Result<ImageThumbnail> {
        let permit = self
            .thumbnail_workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                ScannerError::new(
                    "resourceLimit",
                    "At most two thumbnails may be generated concurrently.",
                )
            })?;
        let engine = self.clone();
        tokio::task::spawn_blocking(move || {
            // A dropped caller cannot release capacity while decoding still runs.
            let _permit = permit;
            let record = engine.scan_status(job)?;
            let image = selected(&record, sequence, edit)?;
            let bytes = engine.root.read_image(&record, image, 32 * 1024 * 1024)?;
            let encoded = scanner_images::thumbnail::thumbnail(
                &bytes,
                512,
                matches!(format, ThumbnailFormat::Jpeg),
            )?;
            Ok(ImageThumbnail {
                schema_version: 1,
                base64: STANDARD.encode(encoded.bytes),
                sha256: encoded.sha256,
                media_type: encoded.media_type.into(),
                width: encoded.width,
                height: encoded.height,
            })
        })
        .await
        .map_err(worker)?
    }
}
fn metadata(job_id: Uuid, image: &OriginalImage) -> ImageMetadata {
    ImageMetadata {
        schema_version: 1,
        job_id,
        image: image.clone(),
    }
}
pub(crate) fn selected(
    record: &ScanRecord,
    sequence: u32,
    edit: Option<Uuid>,
) -> Result<&OriginalImage> {
    if !(1..=1000).contains(&sequence) {
        return Err(ScannerError::new(
            "invalidParams",
            "Invalid image sequence.",
        ));
    }
    let image = match edit {
        Some(id) => record
            .processed_images
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|e| e.request.id == id && e.request.sequence == sequence)
            .map(|e| &e.image),
        None => record.images.iter().find(|i| i.sequence == sequence),
    };
    image.ok_or_else(|| {
        ScannerError::new("imageNotFound", "Requested original or edit was not found.")
    })
}
pub(crate) fn worker(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("image", error.to_string())
}
