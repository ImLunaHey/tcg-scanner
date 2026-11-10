use crate::{StorageRoot, paths, storage};
use scanner_types::{error::Result, scan::ScanRecord};
use std::{io::Read, path::Path};
impl StorageRoot {
    /// Read only the manifest in the record's pinned batch directory. Identity
    /// and progression checks belong to the coordinator before reconciliation.
    pub fn read_manifest(&self, record: &ScanRecord) -> Result<ScanRecord> {
        self.validate()?;
        let directory = self.batch_dir(record)?;
        let file = paths::read_file(&directory, Path::new("manifest.json"))?;
        let mut bytes = vec![];
        file.take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(storage)?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(storage("Manifest exceeds 16 MiB."));
        }
        serde_json::from_slice(&bytes).map_err(storage)
    }
}
