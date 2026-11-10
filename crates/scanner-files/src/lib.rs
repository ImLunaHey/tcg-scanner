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
    dir: Dir,
    path: PathBuf,
    policy: Option<configuration::StorageConfiguration>,
}

impl StorageRoot {
    pub fn open(path: &Path) -> Result<Self> {
        let path = path.canonicalize().map_err(storage)?;
        let dir = Dir::open_ambient_dir(&path, cap_std::ambient_authority()).map_err(storage)?;
        Ok(Self {
            dir,
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
        paths::descend(&self.dir, relative)
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
