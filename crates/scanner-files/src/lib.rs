mod access;
mod batch;
pub mod configuration;
mod derived;
mod manifest;
mod metadata;
mod paths;
pub use access::ImageChunk;
pub use batch::BatchStore;

use cap_std::fs::Dir;
use scanner_types::{
    error::{Result, ScannerError},
    scan::ScanRecord,
};
use std::path::{Path, PathBuf};

/// A pinned capability: every batch operation stays beneath this directory.
pub struct StorageRoot {
    dir: std::sync::Mutex<Option<Dir>>,
    path: PathBuf,
    policy: Option<configuration::StorageConfiguration>,
}

impl StorageRoot {
    pub fn open(path: &Path) -> Result<Self> {
        let path = path.canonicalize().map_err(storage)?;
        let dir = Dir::open_ambient_dir(&path, cap_std::ambient_authority()).map_err(storage)?;
        Ok(Self {
            dir: std::sync::Mutex::new(Some(dir)),
            path,
            policy: None,
        })
    }
    pub fn open_configured(policy: configuration::StorageConfiguration) -> Result<Self> {
        policy.validate()?;
        let mut root = Self::open(Path::new(&policy.storage_root))?;
        root.policy = Some(policy);
        Ok(root)
    }
    /// Construct a configured capability without touching an unavailable volume.
    /// The first operation opens it only after mount-policy validation succeeds.
    pub fn deferred(policy: configuration::StorageConfiguration) -> Result<Self> {
        policy.validate_shape()?;
        Ok(Self {
            dir: std::sync::Mutex::new(None),
            path: PathBuf::from(&policy.storage_root),
            policy: Some(policy),
        })
    }
    pub fn status(&self) -> scanner_types::service::StorageStatus {
        let result = self.check_available();
        scanner_types::service::StorageStatus {
            schema_version: 1,
            storage_root: self.path.to_string_lossy().into(),
            required_smb_share: self
                .policy
                .as_ref()
                .and_then(|p| p.required_smb_share.clone()),
            available: result.is_ok(),
            error: result.err(),
        }
    }
    pub fn check_available(&self) -> Result<()> {
        self.directory()?.dir_metadata().map_err(unavailable)?;
        Ok(())
    }
    /// Replace the pinned handle after an explicit idle-service reconnect. This
    /// does not mount shares, create directories or clear coordinator faults.
    pub fn reconnect(&self) -> Result<()> {
        self.validate()?;
        let directory =
            Dir::open_ambient_dir(&self.path, cap_std::ambient_authority()).map_err(unavailable)?;
        *self.dir.lock().map_err(storage)? = Some(directory);
        Ok(())
    }
    fn directory(&self) -> Result<Dir> {
        self.validate()?;
        let mut slot = self.dir.lock().map_err(storage)?;
        if slot.is_none() {
            *slot = Some(
                Dir::open_ambient_dir(&self.path, cap_std::ambient_authority())
                    .map_err(unavailable)?,
            );
        }
        slot.as_ref().unwrap().try_clone().map_err(unavailable)
    }
    fn validate(&self) -> Result<()> {
        self.policy.as_ref().map(|p| p.validate()).unwrap_or(Ok(()))
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    fn batch_dir(&self, record: &ScanRecord) -> Result<Dir> {
        let relative = Path::new(&record.output_directory)
            .strip_prefix(&self.path)
            .map_err(storage)?;
        paths::descend(&self.directory()?, relative)
    }
    pub fn save_manifest(&self, record: &ScanRecord) -> Result<()> {
        self.validate()?;
        let directory = self.batch_dir(record)?;
        batch::write_manifest(&directory, record)
    }
}

fn storage(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("storage", error.to_string())
}

fn unavailable(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new(
        "storageUnavailable",
        format!("Configured scan storage is unavailable: {error}"),
    )
}

#[cfg(test)]
mod availability;
