use crate::{StorageRoot, paths, storage};
use cap_fs_ext::DirExt;
use cap_std::fs::Dir;
use scanner_types::{
    Extensions,
    device::Device,
    error::Result,
    images::OriginalImage,
    scan::{ScanRecord, ScanRequest, ScanState},
};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[path = "continuation.rs"]
mod continuation;

/// Owns a unique batch and retains staged driver output on failure.
pub struct BatchStore {
    dir: Dir,
    incoming: Dir,
    incoming_path: PathBuf,
    record: ScanRecord,
    starting_count: u32,
}

impl BatchStore {
    pub fn create(
        root: &StorageRoot,
        device: Device,
        request: ScanRequest,
        number: u64,
    ) -> Result<Self> {
        request.validate()?;
        if number == 0 {
            return Err(storage("Batch number must be positive."));
        }
        let (parent, path) = parent(root, &request)?;
        let name = format!("B-{number:03}");
        // create_dir is exclusive: an existing batch is never reused or overwritten.
        paths::create_directory(&parent, Path::new(&name)).map_err(storage)?;
        let dir = parent.open_dir_nofollow(&name).map_err(storage)?;
        paths::create_directory(&dir, Path::new(".incoming")).map_err(storage)?;
        let incoming = dir.open_dir_nofollow(".incoming").map_err(storage)?;
        let directory = path.join(name);
        let mut record = ScanRecord::new(device, request, directory.to_string_lossy().into());
        record.batch_number = Some(number);
        write_manifest(&dir, &record)?;
        Ok(Self {
            dir,
            incoming,
            incoming_path: directory.join(".incoming"),
            record,
            starting_count: 0,
        })
    }
    pub fn incoming_path(&self) -> &Path {
        &self.incoming_path
    }
    pub fn record(&self) -> &ScanRecord {
        &self.record
    }
    pub fn update(&mut self, change: impl FnOnce(&mut ScanRecord)) -> Result<()> {
        let mut next = self.record.clone();
        change(&mut next);
        self.update_continuation(&mut next);
        write_manifest(&self.dir, &next)?;
        self.record = next;
        Ok(())
    }
    pub fn preserve(&mut self, filename: &str, callback_sequence: u32) -> Result<OriginalImage> {
        let sequence = self
            .starting_count
            .checked_add(callback_sequence)
            .ok_or_else(|| storage("Invalid capture sequence."))?;
        if !(1..=100).contains(&callback_sequence)
            || self.record.images.iter().any(|i| i.sequence == sequence)
            || paths::components(Path::new(filename))?.len() != 1
        {
            return Err(storage(
                "Invalid, duplicate or excess capture; staging retained.",
            ));
        }
        let mut source = paths::read_file(&self.incoming, Path::new(filename))?;
        let size = source.metadata().map_err(storage)?.len();
        if size == 0 || size > 512 * 1024 * 1024 {
            return Err(storage(
                "Driver output exceeds the source size limit; staging retained.",
            ));
        }
        let metadata = crate::metadata::inspect(&mut source, self.record.request.format)?;
        source.seek(SeekFrom::Start(0)).map_err(storage)?;
        let name = format!("C-{sequence:03}.{}", self.record.request.format.extension());
        let mut destination = paths::create_file(&self.dir, &name)?;
        let digest = copy_hashed(&mut source, &mut destination, size)?;
        destination.sync_all().map_err(storage)?;
        destination.seek(SeekFrom::Start(0)).map_err(storage)?;
        let copied = hash_reader(&mut destination)?;
        if copied != digest {
            return Err(storage("Saved source checksum mismatch; staging retained."));
        }
        let image = OriginalImage {
            sequence,
            relative_path: name,
            bytes: size,
            sha256: digest,
            width: metadata.width,
            height: metadata.height,
            image_count: metadata.count,
            uniform_type: metadata.uniform_type.into(),
            side: "unknown".into(),
            extensions: Extensions::new(),
        };
        self.update(|record| {
            record.images.push(image.clone());
            record.images.sort_by_key(|i| i.sequence);
        })?;
        Ok(image)
    }
    pub fn finish(
        &mut self,
        errors: Vec<scanner_types::error::ScannerError>,
        cancelled: bool,
        closed: bool,
    ) -> Result<()> {
        let starting_count = self.starting_count;
        self.update(|record| {
            record.errors.extend(errors);
            record.session_closed = closed;
            if record.images.len() == starting_count as usize
                && record.errors.is_empty()
                && !cancelled
            {
                record.errors.push(scanner_types::error::ScannerError::new(
                    "noImages",
                    "Scanner completed without delivering images.",
                ));
            }
            if record.request.duplex && !record.images.len().is_multiple_of(2) {
                record
                    .warnings
                    .push("Odd duplex image count; do not pair automatically.".into());
            }
            record.state = if !record.errors.is_empty() {
                ScanState::Failed
            } else if cancelled {
                ScanState::Cancelled
            } else {
                ScanState::Completed
            };
            record.completed_at = Some(chrono::Utc::now());
        })
    }
}

fn parent(root: &StorageRoot, request: &ScanRequest) -> Result<(Dir, PathBuf)> {
    let Some(output) = request.output_directory.as_ref() else {
        return Ok((root.dir.try_clone().map_err(storage)?, root.path.clone()));
    };
    let output = Path::new(output);
    let relative = if output.is_absolute() {
        output.strip_prefix(&root.path).map_err(storage)?
    } else {
        output
    };
    if relative.as_os_str().is_empty() {
        return Ok((root.dir.try_clone().map_err(storage)?, root.path.clone()));
    }
    let mut dir = root.dir.try_clone().map_err(storage)?;
    for part in paths::components(relative)? {
        match paths::create_directory(&dir, Path::new(part)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(storage(e)),
        }
        dir = dir.open_dir_nofollow(part).map_err(storage)?;
    }
    Ok((dir, root.path.join(relative)))
}

pub(super) fn write_manifest(dir: &Dir, record: &ScanRecord) -> Result<()> {
    if let Ok(metadata) = dir.symlink_metadata("manifest.json")
        && (!metadata.is_file() || metadata.file_type().is_symlink())
    {
        return Err(storage("Manifest is not a regular file."));
    }
    let name = format!(".manifest-{}.tmp", Uuid::new_v4());
    let mut file = paths::create_file(dir, &name)?;
    file.write_all(&serde_json::to_vec_pretty(record).map_err(storage)?)
        .map_err(storage)?;
    file.sync_all().map_err(storage)?;
    drop(file);
    dir.rename(&name, dir, "manifest.json").map_err(storage)?;
    #[cfg(unix)]
    {
        // Some network filesystems do not implement directory fsync. File contents
        // were synced above; all other directory-sync errors must be reported.
        if let Err(error) = dir.try_clone().map_err(storage)?.into_std_file().sync_all()
            && error.kind() != std::io::ErrorKind::Unsupported
        {
            return Err(storage(error));
        }
    }
    Ok(())
}

fn copy_hashed(
    source: &mut impl Read,
    destination: &mut impl Write,
    expected: u64,
) -> Result<String> {
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut count = 0_u64;
    loop {
        let n = source.read(&mut buffer).map_err(storage)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > expected {
            return Err(storage("Driver output grew while being copied."));
        }
        hash.update(&buffer[..n]);
        destination.write_all(&buffer[..n]).map_err(storage)?;
    }
    if count != expected {
        return Err(storage("Driver output changed while being copied."));
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn hash_reader(reader: &mut impl Read) -> Result<String> {
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer).map_err(storage)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_driver_bytes_and_never_overwrites_existing_batches_or_images() {
        let temp = tempfile::tempdir().unwrap();
        let root = StorageRoot::open(temp.path()).unwrap();
        let request = ScanRequest::default();
        let mut batch = BatchStore::create(&root, Device::default(), request.clone(), 1).unwrap();
        let image = scanner_images::encode_png(image::RgbaImage::from_pixel(
            60,
            90,
            image::Rgba([40, 90, 120, 255]),
        ))
        .unwrap();
        std::fs::write(batch.incoming_path().join("capture.png"), &image.bytes).unwrap();
        let saved = batch.preserve("capture.png", 1).unwrap();
        assert_eq!(
            root.read_image(batch.record(), &saved, 1024 * 1024)
                .unwrap(),
            image.bytes
        );
        assert!(batch.incoming_path().join("capture.png").exists());
        assert!(batch.preserve("capture.png", 1).is_err());
        assert!(BatchStore::create(&root, Device::default(), request, 1).is_err());
        batch.finish(vec![], false, true).unwrap();
        assert_eq!(batch.record.state, ScanState::Completed);
    }
}
