use crate::backend::CardProcessor;
use async_trait::async_trait;
use scanner_files::StorageRoot;
use scanner_recognition::{ocr::Tesseract, orientation::Orientation};
use scanner_types::{
    Extensions,
    cards::CardLanguage,
    error::{Result, ScannerError},
    images::{ImageEditRequest, OriginalImage, ProcessedImage},
    scan::{ScanBackground, ScanRecord},
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;
use uuid::Uuid;

/// Bounded native image preparation. Paths are provided by application packaging,
/// never inferred from a platform-specific development installation.
pub struct NativeCardProcessor {
    root: Arc<StorageRoot>,
    library: PathBuf,
    trained_data: PathBuf,
    workers: Arc<Semaphore>,
}
impl NativeCardProcessor {
    pub fn new(root: Arc<StorageRoot>, library: PathBuf, trained_data: PathBuf) -> Self {
        Self {
            root,
            library,
            trained_data,
            workers: Arc::new(Semaphore::new(2)),
        }
    }
}
#[async_trait]
impl CardProcessor for NativeCardProcessor {
    async fn prepare(&self, record: &ScanRecord, sequence: u32) -> Result<ProcessedImage> {
        self.prepare_with_id(record, sequence, Uuid::new_v4()).await
    }
    async fn prepare_with_id(
        &self,
        record: &ScanRecord,
        sequence: u32,
        id: Uuid,
    ) -> Result<ProcessedImage> {
        let permit = self
            .workers
            .clone()
            .acquire_owned()
            .await
            .map_err(processing)?;
        let root = self.root.clone();
        let record = record.clone();
        let library = self.library.clone();
        let data = self.trained_data.clone();
        tokio::task::spawn_blocking(move || {
            // Retain the permit even if the caller disconnects while native work runs.
            let _permit = permit;
            let language = match record.request.card_language.unwrap_or_default() {
                CardLanguage::English => "en",
                CardLanguage::Korean => "ko",
            };
            let mut engine = Tesseract::new(&library, &data, language)?;
            prepare(&root, &record, sequence, id, |pixels| {
                engine.orientation(pixels)
            })
        })
        .await
        .map_err(processing)?
    }
}
fn processing(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("image", error.to_string())
}

fn prepare(
    root: &StorageRoot,
    record: &ScanRecord,
    sequence: u32,
    id: Uuid,
    orient: impl FnOnce(&image::RgbImage) -> Result<Orientation>,
) -> Result<ProcessedImage> {
    let original = record
        .images
        .iter()
        .find(|i| i.sequence == sequence)
        .ok_or_else(|| ScannerError::new("imageNotFound", "Original image not found."))?;
    if original.image_count != 1 {
        return Err(processing(
            "Card preparation requires exactly one image frame.",
        ));
    }
    let bytes = root.read_image(record, original, 32 * 1024 * 1024)?;
    if let Some(edit) = root.recover_preparation(record, id, sequence)? {
        return Ok(edit);
    }
    let mut plan = scanner_images::boundary::prepare_geometry(
        &bytes,
        record.request.background.unwrap_or(ScanBackground::Black),
    )?;
    let mut request = ImageEditRequest {
        id,
        job_id: record.id,
        sequence,
        crop: plan.crop,
        quarter_turns: 0,
        deskew_degrees: plan.deskew_degrees,
        extensions: Extensions::new(),
    };
    let source = scanner_images::decode::decode(&bytes)?.to_rgba8();
    let cropped = scanner_images::edit::render_pixels(&source, &request)?;
    drop(source);
    let orientation = orient(
        &image::DynamicImage::ImageRgba8(cropped.clone())
            .thumbnail(1200, 1200)
            .to_rgb8(),
    )?;
    apply_orientation(&mut plan, &orientation);
    request.quarter_turns = orientation.quarter_turns;
    let upright = match request.quarter_turns {
        1 => image::imageops::rotate90(&cropped),
        2 => image::imageops::rotate180(&cropped),
        3 => image::imageops::rotate270(&cropped),
        _ => cropped,
    };
    let encoded = scanner_images::encode_png(upright)?;
    let edit = ProcessedImage {
        image: OriginalImage {
            sequence,
            relative_path: format!("edit-{}.png", request.id),
            bytes: encoded.bytes.len() as u64,
            sha256: encoded.sha256,
            width: encoded.width,
            height: encoded.height,
            image_count: 1,
            uniform_type: "public.png".into(),
            side: original.side.clone(),
            extensions: Extensions::new(),
        },
        request,
        created_at: chrono::Utc::now(),
        source_sha256: original.sha256.clone(),
        card_preparation: Some(plan),
        extensions: Extensions::new(),
    };
    root.write_edit(record, &edit, &encoded.bytes)?;
    Ok(edit)
}

fn apply_orientation(plan: &mut scanner_types::images::CardPreparation, orientation: &Orientation) {
    plan.quarter_turns = orientation.quarter_turns;
    plan.orientation_verified = orientation.verified;
    plan.orientation_basis = Some(orientation.basis.into());
    plan.method = "card-boundary-tesseract-rust-v1".into();
    plan.warnings
        .retain(|w| !w.starts_with("Orientation has not yet"));
    if !orientation.verified {
        plan.warnings.push(
            "Orientation is uncertain; review the saved copy before confirming the card.".into(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scanner_files::BatchStore;
    use scanner_types::{device::Device, scan::ScanRequest};
    #[test]
    fn uncertain_orientation_preserves_original_and_publishes_a_separate_crop() {
        let directory = tempfile::tempdir().unwrap();
        let root = StorageRoot::open(directory.path()).unwrap();
        let mut store =
            BatchStore::create(&root, Device::default(), ScanRequest::default(), 1).unwrap();
        let mut pixels = image::RgbaImage::from_pixel(300, 400, image::Rgba([0, 0, 0, 255]));
        for y in 70..330 {
            for x in 60..240 {
                pixels.put_pixel(x, y, image::Rgba([180, 130, 40, 255]));
            }
        }
        let encoded = scanner_images::encode_png(pixels).unwrap();
        std::fs::write(store.incoming_path().join("driver.png"), &encoded.bytes).unwrap();
        store.preserve("driver.png", 1).unwrap();
        let id = Uuid::new_v4();
        let edit = prepare(&root, store.record(), 1, id, |_| {
            Ok(Orientation {
                quarter_turns: 0,
                verified: false,
                basis: "unknown",
            })
        })
        .unwrap();
        assert_eq!(
            root.read_image(store.record(), &store.record().images[0], 32 * 1024 * 1024)
                .unwrap(),
            encoded.bytes
        );
        let recovered = prepare(&root, store.record(), 1, id, |_| {
            panic!("Recovered preparation must not rerun OCR")
        })
        .unwrap();
        assert_eq!(recovered.request.id, id);
        assert_eq!(recovered.image.sha256, edit.image.sha256);
        assert_eq!(edit.source_sha256, encoded.sha256);
        assert!(!edit.card_preparation.as_ref().unwrap().orientation_verified);
        assert!(edit.image.width < 300 && edit.image.height < 400);
        let mut published = store.record().clone();
        published.processed_images = Some(vec![edit.clone()]);
        let saved = root
            .read_image(&published, &edit.image, 64 * 1024 * 1024)
            .unwrap();
        assert_eq!(
            scanner_images::decode::decode(&saved).unwrap().width(),
            edit.image.width
        );
    }
}
