use crate::{StorageRoot, paths, storage};
use scanner_types::{error::Result, images::OriginalImage, scan::ScanRecord};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub struct ImageChunk {
    pub bytes: Vec<u8>,
    pub offset: u64,
    pub sha256: String,
    pub eof: bool,
}

impl StorageRoot {
    pub fn read_image(
        &self,
        record: &ScanRecord,
        image: &OriginalImage,
        limit: u64,
    ) -> Result<Vec<u8>> {
        if image.bytes > limit || limit > 512 * 1024 * 1024 {
            return Err(storage("Source exceeds the requested byte limit."));
        }
        let mut file = self.open_image(record, image)?;
        let mut bytes = Vec::with_capacity(image.bytes as usize);
        (&mut file)
            .take(image.bytes + 1)
            .read_to_end(&mut bytes)
            .map_err(storage)?;
        if bytes.len() as u64 != image.bytes
            || format!("{:x}", Sha256::digest(&bytes)) != image.sha256
        {
            return Err(storage(
                "Saved image checksum or byte count does not match its record.",
            ));
        }
        Ok(bytes)
    }
    pub fn read_chunk(
        &self,
        record: &ScanRecord,
        image: &OriginalImage,
        offset: u64,
        length: usize,
    ) -> Result<ImageChunk> {
        if offset > image.bytes || !(1..=262_144).contains(&length) {
            return Err(storage("Invalid image offset or chunk length."));
        }
        let mut file = self.open_image(record, image)?;
        file.seek(SeekFrom::Start(offset)).map_err(storage)?;
        let count = (image.bytes - offset).min(length as u64) as usize;
        let mut bytes = vec![0; count];
        file.read_exact(&mut bytes).map_err(storage)?;
        Ok(ImageChunk {
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            bytes,
            offset,
            eof: offset + count as u64 == image.bytes,
        })
    }
    fn open_image(&self, record: &ScanRecord, image: &OriginalImage) -> Result<cap_std::fs::File> {
        validate_name(record, image)?;
        let directory = self.batch_dir(record)?;
        let file = paths::read_file(&directory, Path::new(&image.relative_path))?;
        if file.metadata().map_err(storage)?.len() != image.bytes || image.bytes > 512 * 1024 * 1024
        {
            return Err(storage("Saved image size does not match its record."));
        }
        Ok(file)
    }
}

fn validate_name(record: &ScanRecord, image: &OriginalImage) -> Result<()> {
    let original = format!(
        "C-{:03}.{}",
        image.sequence,
        record.request.format.extension()
    );
    let legacy = format!(
        "page-{:06}.{}",
        image.sequence,
        record.request.format.extension()
    );
    let is_original = record.images.iter().any(|r| {
        r.sequence == image.sequence
            && r.sha256 == image.sha256
            && r.relative_path == image.relative_path
    }) && [original, legacy.clone(), format!("originals/{legacy}")]
        .contains(&image.relative_path);
    let is_edit = record
        .processed_images
        .as_ref()
        .into_iter()
        .flatten()
        .any(|edit| {
            edit.image.sha256 == image.sha256
                && edit.request.sequence == image.sequence
                && edit.image.relative_path == image.relative_path
                && [
                    format!("edit-{}.png", edit.request.id),
                    format!("processed/{}.png", edit.request.id),
                ]
                .contains(&image.relative_path)
        });
    if !(is_original || is_edit) {
        return Err(storage("Image path is not a recorded original or edit."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use scanner_types::{Extensions, device::Device, scan::ScanRequest};
    #[test]
    fn reads_existing_saved_originals_and_edits_without_modifying_the_batch() {
        let Some(path) = std::env::var_os("SCANNER_COMPAT_MANIFEST") else {
            return;
        };
        let record: ScanRecord = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let parent = Path::new(&record.output_directory).parent().unwrap();
        let root = StorageRoot::open(parent).unwrap();
        for image in record
            .images
            .iter()
            .take(2)
            .chain(record.images.last())
            .chain(
                record
                    .processed_images
                    .as_ref()
                    .and_then(|v| v.last())
                    .map(|v| &v.image),
            )
        {
            assert_eq!(
                root.read_image(&record, image, 512 * 1024 * 1024)
                    .unwrap()
                    .len() as u64,
                image.bytes
            );
        }
    }
    #[test]
    fn verifies_sources_and_rejects_same_size_tampering() {
        let root = tempfile::tempdir().unwrap();
        let batch = root.path().join("B-001");
        std::fs::create_dir(&batch).unwrap();
        let batch = batch.canonicalize().unwrap();
        let mut record = ScanRecord::new(
            Device::default(),
            ScanRequest::default(),
            batch.to_string_lossy().into(),
        );
        let image = OriginalImage {
            sequence: 1,
            relative_path: "C-001.png".into(),
            bytes: 3,
            sha256: format!("{:x}", Sha256::digest(b"abc")),
            width: 1,
            height: 1,
            image_count: 1,
            uniform_type: "public.png".into(),
            side: "unknown".into(),
            extensions: Extensions::new(),
        };
        record.images.push(image.clone());
        std::fs::write(batch.join("C-001.png"), b"abc").unwrap();
        let storage = StorageRoot::open(root.path()).unwrap();
        assert_eq!(storage.read_image(&record, &image, 1024).unwrap(), b"abc");
        assert_eq!(
            storage.read_chunk(&record, &image, 1, 2).unwrap().bytes,
            b"bc"
        );
        std::fs::write(batch.join("C-001.png"), b"abd").unwrap();
        assert!(storage.read_image(&record, &image, 1024).is_err());
        #[cfg(unix)]
        {
            std::fs::remove_file(batch.join("C-001.png")).unwrap();
            std::os::unix::fs::symlink("/etc/passwd", batch.join("C-001.png")).unwrap();
            assert!(storage.read_image(&record, &image, 1024).is_err());
        }
    }
    #[test]
    fn rejects_directory_symlinks_and_forged_paths() {
        let root = tempfile::tempdir().unwrap();
        let storage = StorageRoot::open(root.path()).unwrap();
        let mut record = ScanRecord::new(
            Device::default(),
            ScanRequest::default(),
            storage.path().join("../outside").to_string_lossy().into(),
        );
        assert!(storage.save_manifest(&record).is_err());
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            std::os::unix::fs::symlink(outside.path(), root.path().join("B-001")).unwrap();
            record.output_directory = storage.path().join("B-001").to_string_lossy().into();
            assert!(storage.save_manifest(&record).is_err());
            assert!(!outside.path().join("manifest.json").exists());
        }
    }
}
