mod access;
mod batch;
mod derived;
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
}

impl StorageRoot {
    pub fn open(path: &Path) -> Result<Self> {
        let path = path.canonicalize().map_err(storage)?;
        let dir = Dir::open_ambient_dir(&path, cap_std::ambient_authority()).map_err(storage)?;
        Ok(Self { dir, path })
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
        let directory = self.batch_dir(record)?;
        batch::write_manifest(&directory, record)
    }
}

fn storage(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("storage", error.to_string())
}
