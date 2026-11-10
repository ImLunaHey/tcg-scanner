use crate::{
    Engine,
    image_access::{selected, worker},
};
use scanner_types::{
    Extensions,
    error::{Result, ScannerError},
    images::{ImageEditRequest, OriginalImage, ProcessedImage},
    scan::ScanRecord,
};
use std::sync::Arc;

impl Engine {
    pub async fn edit_image(self: &Arc<Self>, request: ImageEditRequest) -> Result<ProcessedImage> {
        request.validate()?;
        let permit = self
            .edit_workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| ScannerError::new("busy", "Two image edits are already pending."))?;
        let engine = self.clone();
        tokio::spawn(async move {
            let _permit = permit;
            // Serialize publication with continuation/default changes and make shutdown
            // wait for accepted edits. The owned task survives caller disconnection.
            let _reservation = engine.submissions.lock().await;
            engine.require_saved_job(request.job_id).await?;
            let worker_engine = engine.clone();
            tokio::task::spawn_blocking(move || worker_engine.edit_owned(request))
                .await
                .map_err(worker)?
        })
        .await
        .map_err(worker)?
    }
    fn edit_owned(&self, request: ImageEditRequest) -> Result<ProcessedImage> {
        let mut record = self.scan_status(request.job_id)?;
        if let Some(edit) = record
            .processed_images
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|e| e.request.id == request.id)
        {
            if edit.request != request {
                return Err(ScannerError::new(
                    "conflict",
                    "Edit ID already has different inputs.",
                ));
            }
            self.root
                .read_image(&record, &edit.image, 64 * 1024 * 1024)?;
            self.root.save_manifest(&record)?;
            return Ok(edit.clone());
        }
        if !record.state.terminal() {
            return Err(ScannerError::new(
                "busy",
                "Wait for the scan to finish before editing.",
            ));
        }
        if record
            .processed_images
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter(|e| e.request.sequence == request.sequence)
            .count()
            >= 8
        {
            return Err(ScannerError::new(
                "resourceLimit",
                "At most eight saved edits per original are supported.",
            ));
        }
        let original = selected(&record, request.sequence, None)?;
        // Verify the source even when recovering an already encoded edit.
        let bytes = self.root.read_image(&record, original, 32 * 1024 * 1024)?;
        let edit = if let Some(edit) = self.root.recover_edit(&record, &request)? {
            edit
        } else {
            self.render_edit(&record, request, &bytes, original)?
        };
        record
            .processed_images
            .get_or_insert_with(Vec::new)
            .push(edit.clone());
        self.database.save(&record)?;
        self.root.save_manifest(&record)?;
        Ok(edit)
    }
    fn render_edit(
        &self,
        record: &ScanRecord,
        request: ImageEditRequest,
        bytes: &[u8],
        original: &OriginalImage,
    ) -> Result<ProcessedImage> {
        if original.image_count != 1 {
            return Err(ScannerError::new(
                "image",
                "Editing requires exactly one image frame.",
            ));
        }
        let encoded = scanner_images::edit::render(bytes, &request)?;
        let edit = ProcessedImage {
            image: OriginalImage {
                sequence: request.sequence,
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
            card_preparation: None,
            extensions: Extensions::new(),
        };
        self.root.write_edit(record, &edit, &encoded.bytes)?;
        Ok(edit)
    }
}
