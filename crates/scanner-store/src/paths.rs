use scanner_platform::PrivateDirectory;
use scanner_types::error::{Result, ScannerError};
use std::path::Path;

pub fn prepare(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| ScannerError::new("accessDenied", "Missing database directory."))?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| ScannerError::new("accessDenied", "Invalid database filename."))?;
    let directory = PrivateDirectory::open(parent, true)?;
    for suffix in ["", "-wal", "-shm", "-journal"] {
        directory.validate_existing_file(&format!("{name}{suffix}"))?;
    }
    directory.open_file(name, true, false, true)?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn refuses_symlinked_database_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        use std::os::unix::fs::PermissionsExt;
        let path = dir.path().canonicalize().unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let db = path.join("ledger.sqlite");
        std::os::unix::fs::symlink("/tmp/unused-scanner-db", path.join("ledger.sqlite-wal"))
            .unwrap();
        assert!(super::prepare(&db).is_err());
        assert!(!db.exists());
    }
}
