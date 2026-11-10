use crate::{PrivateDirectory, application_data, error};
use scanner_types::error::{Result, ScannerError};
use std::{fs::File, path::Path};

/// Process-lifetime exclusive owner. Default location is independent of database
/// and listener paths and matches the installed Swift service on macOS.
pub struct HardwareOwner {
    _file: File,
}
impl HardwareOwner {
    pub fn acquire() -> Result<Self> {
        Self::at(&application_data()?.join("run/hardware.lock"))
    }
    /// Explicit location for tests or a separately isolated user installation.
    pub fn at(path: &Path) -> Result<Self> {
        let parent = path
            .parent()
            .ok_or_else(|| error("Missing owner-lock directory."))?;
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| error("Invalid owner-lock filename."))?;
        let directory = PrivateDirectory::open(parent, true)?;
        let file = directory.open_file(name, true, false, true)?;
        file.try_lock().map_err(|_| {
            ScannerError::new(
                "busy",
                "Another scanner service owns hardware access for this user.",
            )
        })?;
        Ok(Self { _file: file })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owner_is_exclusive_until_the_native_handle_closes() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp
            .path()
            .canonicalize()
            .unwrap()
            .join("run/hardware.lock");
        let first = HardwareOwner::at(&path).unwrap();
        assert!(HardwareOwner::at(&path).is_err());
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let swift_style = File::options().read(true).write(true).open(&path).unwrap();
            // SAFETY: the borrowed descriptor stays open throughout the flock call.
            assert_ne!(
                unsafe { libc::flock(swift_style.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
                0
            );
        }
        drop(first);
        assert!(HardwareOwner::at(&path).is_ok());
    }
}
