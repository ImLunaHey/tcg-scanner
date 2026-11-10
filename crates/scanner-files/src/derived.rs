use crate::{StorageRoot, paths, storage};
use scanner_types::{
    error::{Result, ScannerError},
    images::{ImageEditRequest, ProcessedImage},
    scan::ScanRecord,
};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::Path,
};

impl StorageRoot {
    /// Derived images are immutable; metadata publication follows the successful write.
    pub fn write_edit(
        &self,
        record: &ScanRecord,
        edit: &ProcessedImage,
        bytes: &[u8],
    ) -> Result<()> {
        self.validate()?;
        edit.request.validate()?;
        let expected = format!("edit-{}.png", edit.request.id);
        if edit.request.job_id != record.id
            || edit.image.sequence != edit.request.sequence
            || edit.image.relative_path != expected
            || bytes.len() > 64 * 1024 * 1024
            || edit.image.bytes != bytes.len() as u64
            || format!("{:x}", Sha256::digest(bytes)) != edit.image.sha256
            || !record
                .images
                .iter()
                .any(|i| i.sequence == edit.request.sequence && i.sha256 == edit.source_sha256)
        {
            return Err(storage(
                "Derived image metadata does not match its source or encoded bytes.",
            ));
        }
        let directory = self.batch_dir(record)?;
        let mut file = paths::create_file(&directory, &expected)?;
        file.write_all(bytes).map_err(storage)?;
        file.sync_all().map_err(storage)?;
        let metadata = serde_json::to_vec(edit).map_err(storage)?;
        if metadata.len() > 65536 {
            return Err(storage(
                "Edit metadata exceeds 64 KiB; encoded image retained.",
            ));
        }
        let mut sidecar =
            paths::create_file(&directory, &format!(".edit-{}.json", edit.request.id))?;
        sidecar.write_all(&metadata).map_err(storage)?;
        sidecar.sync_all().map_err(storage)?;
        #[cfg(unix)]
        directory.into_std_file().sync_all().map_err(storage)?;
        Ok(())
    }

    /// Recover an immutable edit saved before an interrupted ledger publication.
    /// Partial or mismatched outputs are retained and never overwritten.
    pub fn recover_edit(
        &self,
        record: &ScanRecord,
        request: &ImageEditRequest,
    ) -> Result<Option<ProcessedImage>> {
        request.validate()?;
        let directory = self.batch_dir(record)?;
        let name = format!(".edit-{}.json", request.id);
        match directory.symlink_metadata(&name) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(storage(e)),
            Ok(_) => (),
        }
        let file = paths::read_file(&directory, Path::new(&name))?;
        let mut bytes = Vec::new();
        file.take(65537).read_to_end(&mut bytes).map_err(storage)?;
        if bytes.len() > 65536 {
            return Err(storage("Retained edit metadata exceeds 64 KiB."));
        }
        let edit: ProcessedImage = serde_json::from_slice(&bytes).map_err(storage)?;
        let original = record
            .images
            .iter()
            .find(|i| i.sequence == request.sequence)
            .ok_or_else(|| ScannerError::new("imageNotFound", "Original image not found."))?;
        if request.job_id != record.id
            || edit.request != *request
            || edit.source_sha256 != original.sha256
        {
            return Err(ScannerError::new(
                "conflict",
                "Retained edit has different inputs.",
            ));
        }
        if edit.image.sequence != request.sequence
            || edit.image.relative_path != format!("edit-{}.png", request.id)
        {
            return Err(storage("Invalid retained edit metadata."));
        }
        let mut snapshot = record.clone();
        snapshot
            .processed_images
            .get_or_insert_with(Vec::new)
            .push(edit.clone());
        self.read_image(&snapshot, &edit.image, 64 * 1024 * 1024)?;
        Ok(Some(edit))
    }
}
