use crate::{StorageRoot, paths, storage};
use scanner_types::{error::Result, images::ProcessedImage, scan::ScanRecord};
use sha2::{Digest, Sha256};
use std::io::Write;

impl StorageRoot {
    /// Derived images are immutable; metadata publication follows the successful write.
    pub fn write_edit(
        &self,
        record: &ScanRecord,
        edit: &ProcessedImage,
        bytes: &[u8],
    ) -> Result<()> {
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
        file.sync_all().map_err(storage)
    }
}
