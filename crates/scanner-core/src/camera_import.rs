//! Explicit, authenticated review imports. A capture ID is an idempotency key.
use crate::Engine;
use scanner_files::BatchStore;
use scanner_types::{
    camera_import::CameraImport,
    device::Device,
    error::{Result, ScannerError},
    identification::IdentificationState,
    recognition::RecognitionEvidence,
    scan::{ScanRecord, ScanRequest, ScanState},
};
use sha2::{Digest, Sha256};
use std::{io::Write, sync::Arc};

impl Engine {
    pub async fn import_camera(self: &Arc<Self>, capture: CameraImport) -> Result<ScanRecord> {
        capture.validate()?;
        let bytes = crate::camera::decode(&capture.base64)?;
        let digest = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&capture).map_err(storage)?)
        );
        let engine = self.clone();
        // Own the operation through HTTP disconnection, just like scan submission.
        tokio::spawn(async move {
            let _submission = engine.submissions.lock().await;
            engine.state.lock().await.accepting()?;
            let worker = engine.clone();
            tokio::task::spawn_blocking(move || worker.save_camera(capture, bytes, &digest))
                .await
                .map_err(crate::image_access::worker)?
        })
        .await
        .map_err(crate::image_access::worker)?
    }
    fn save_camera(
        &self,
        capture: CameraImport,
        bytes: Vec<u8>,
        digest: &str,
    ) -> Result<ScanRecord> {
        if let Some(previous) = self.database.job(capture.id)? {
            if previous
                .request
                .extensions
                .get("cameraImportSHA256")
                .and_then(|v| v.as_str())
                != Some(digest)
            {
                return Err(ScannerError::new(
                    "conflict",
                    "This capture was already saved with different review details.",
                ));
            }
            if previous.state != ScanState::Completed {
                return Err(ScannerError::new(
                    "storage",
                    "The previous import was interrupted. Its batch is retained for recovery; no duplicate was added.",
                ));
            }
            return Ok(previous);
        }
        let image = scanner_images::camera::guide_for_ocr(&bytes)?.ok_or_else(|| {
            ScannerError::new(
                "invalidParams",
                "The capture contains no usable card detail.",
            )
        })?;
        let mut request = ScanRequest {
            request_id: capture.id,
            prepare_cards: Some(true),
            card_language: Some(capture.selection.language),
            device_id: Some("camera-import".into()),
            ..Default::default()
        };
        request
            .extensions
            .insert("cameraImportSHA256".into(), digest.into());
        let number = self.database.reserve_batch_number(capture.id)?;
        let mut batch = BatchStore::create(
            &self.root,
            Device {
                id: "camera-import".into(),
                name: "Camera capture".into(),
                ..Default::default()
            },
            request.clone(),
            number,
        )?;
        batch.update(|r| r.state = ScanState::Processing)?;
        self.database.insert(batch.record(), &request)?;
        let result = write_copies(&mut batch, &capture, &image.bytes);
        if let Err(error) = result {
            batch.finish(vec![error.clone()], false, true)?;
            self.database.save(batch.record())?;
            return Err(error);
        }
        let games = scanner_types::game_detection::CardGameDetection::resolve(batch.record());
        batch.update(|r| {
            r.extensions
                .insert("cardGames".into(), serde_json::json!(games));
        })?;
        batch.finish(vec![], false, true)?;
        self.database.save(batch.record())?;
        Ok(batch.record().clone())
    }
}
fn write_copies(batch: &mut BatchStore, capture: &CameraImport, png: &[u8]) -> Result<()> {
    let path = batch.incoming_path().join("camera.png");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(storage)?;
    file.write_all(png).map_err(storage)?;
    file.sync_all().map_err(storage)?;
    drop(file);
    for sequence in 1..=capture.quantity {
        let image = batch.preserve("camera.png", sequence)?;
        let mut identity =
            capture
                .selection
                .identification(batch.record().id, sequence, image.sha256);
        identity.state = IdentificationState::Confirmed;
        identity.selected_card_id = Some(capture.selection.card.id.clone());
        identity.completed_at = Some(chrono::Utc::now());
        identity.reviewed_at = identity.completed_at;
        identity.method = "reviewed-camera-capture-v1".into();
        identity.evidence = Some(RecognitionEvidence {
            candidates: vec![capture.selection.card.clone()],
            ..Default::default()
        });
        let mut next = batch.record().clone();
        next.replace_identification(&identity)?;
        next.images.last_mut().unwrap().extensions.insert(
            "reviewedCameraGame".into(),
            serde_json::json!(capture.selection.game),
        );
        batch.update(|record| *record = next)?;
    }
    Ok(())
}
fn storage(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("storage", error.to_string())
}
