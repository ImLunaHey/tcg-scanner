use crate::configuration::config;
use scanner_platform::PrivateDirectory;
use scanner_types::error::Result;
use std::{io::Write, path::Path};

pub(super) fn write(path: &Path, bytes: &[u8], executable: bool) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| config("Missing destination parent."))?;
    let name = path
        .file_name()
        .and_then(|p| p.to_str())
        .ok_or_else(|| config("Invalid destination filename."))?;
    let directory = PrivateDirectory::open(parent, true)?;
    let mut file = directory.open_file(name, true, true, false)?;
    file.write_all(bytes).map_err(config)?;
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o700))
            .map_err(config)?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    file.sync_all().map_err(config)?;
    directory.synchronize()
}
