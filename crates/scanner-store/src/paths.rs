use crate::storage;
use scanner_types::error::{Result, ScannerError};
use std::{fs, path::Path};

pub fn prepare(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    for ancestor in parent.ancestors() {
        if let Ok(metadata) = fs::symlink_metadata(ancestor)
            && metadata.file_type().is_symlink()
        {
            return Err(ScannerError::new(
                "accessDenied",
                "Database ancestors must not be symlinks.",
            ));
        }
    }
    if !parent.exists() {
        fs::create_dir_all(parent).map_err(storage)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).map_err(storage)?;
        }
    }
    private_owned(parent)?;
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        let file = Path::new(&name);
        match fs::symlink_metadata(file) {
            Ok(meta) => {
                if !meta.is_file() || meta.file_type().is_symlink() {
                    return Err(ScannerError::new(
                        "accessDenied",
                        "Database files must be regular files.",
                    ));
                }
                private_owned(file)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(storage(error)),
        }
    }
    if !path.exists() {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(path).map_err(storage)?;
    }
    Ok(())
}

fn private_owned(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = fs::symlink_metadata(path).map_err(storage)?;
        // SAFETY: geteuid has no pointer arguments or side effects.
        if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
            return Err(ScannerError::new(
                "accessDenied",
                "Database storage must be private and owned by the current user.",
            ));
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn refuses_symlinked_database_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("ledger.sqlite");
        std::os::unix::fs::symlink(
            "/tmp/unused-scanner-db",
            dir.path().join("ledger.sqlite-wal"),
        )
        .unwrap();
        assert!(super::prepare(&db).is_err());
        assert!(!db.exists());
    }
}
