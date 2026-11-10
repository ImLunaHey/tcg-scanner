use crate::{PrivateDirectory, error};
use scanner_types::error::Result;
use std::{
    io::{Read, Write},
    path::Path,
};

/// Persist a caller-generated 256-bit hexadecimal token without replacing any file.
/// Token contents are never included in errors or diagnostics.
pub fn create_credential(path: &Path, token: &str) -> Result<()> {
    if !valid(token) {
        return Err(error(
            "Credential must be 64 lowercase hexadecimal characters.",
        ));
    }
    let (parent, name) = location(path)?;
    let directory = PrivateDirectory::open(parent, true)?;
    let mut file = directory.open_file(name, true, true, false)?;
    file.write_all(token.as_bytes()).map_err(error)?;
    file.write_all(b"\n").map_err(error)?;
    file.sync_all().map_err(error)?;
    directory.synchronize()
}
pub fn load_credential(path: &Path) -> Result<String> {
    let (parent, name) = location(path)?;
    let directory = PrivateDirectory::open(parent, false)?;
    let file = directory.open_file(name, false, false, false)?;
    if !(64..=65).contains(&file.metadata().map_err(error)?.len()) {
        return Err(error("Invalid credential file length."));
    }
    let mut bytes = vec![];
    file.take(66).read_to_end(&mut bytes).map_err(error)?;
    if bytes.len() == 65 && bytes[64] == b'\n' {
        bytes.pop();
    }
    let token = String::from_utf8(bytes).map_err(|_| error("Invalid credential encoding."))?;
    if !valid(&token) {
        return Err(error(
            "Credential must be 64 lowercase hexadecimal characters.",
        ));
    }
    Ok(token)
}
fn valid(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn location(path: &Path) -> Result<(&Path, &str)> {
    let parent = path
        .parent()
        .ok_or_else(|| error("Missing credential parent directory."))?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| error("Invalid credential filename."))?;
    Ok((parent, name))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credential_round_trip_is_exclusive_and_rejects_unsafe_files() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().canonicalize().unwrap();
        let path = directory.join("private/token");
        let token = "a".repeat(64);
        create_credential(&path, &token).unwrap();
        assert_eq!(load_credential(&path).unwrap(), token);
        assert!(create_credential(&path, &"b".repeat(64)).is_err());
        assert_eq!(load_credential(&path).unwrap(), token);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(load_credential(&path).is_err());
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            std::fs::hard_link(&path, directory.join("copy")).unwrap();
            assert!(load_credential(&path).is_err());
            std::fs::remove_file(directory.join("copy")).unwrap();
            std::os::unix::fs::symlink(&path, directory.join("private/link")).unwrap();
            assert!(load_credential(&directory.join("private/link")).is_err());
        }
    }
}
