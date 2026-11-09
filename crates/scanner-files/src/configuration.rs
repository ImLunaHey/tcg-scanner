use scanner_types::error::{Result, ScannerError};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageConfiguration {
    pub storage_root: String,
    #[serde(
        default,
        rename = "requiredSMBShare",
        skip_serializing_if = "Option::is_none"
    )]
    pub required_smb_share: Option<String>,
}

impl StorageConfiguration {
    pub fn load(path: &Path, default_root: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(crate::storage),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                storage_root: default_root.to_string_lossy().into(),
                required_smb_share: None,
            }),
            Err(error) => Err(crate::storage(error)),
        }
    }
    pub fn validate(&self) -> Result<()> {
        let root = Path::new(&self.storage_root);
        if !root.is_absolute()
            || self.storage_root.contains('\0')
            || root.components().any(|p| matches!(p, Component::ParentDir))
        {
            return Err(ScannerError::new(
                "invalidSettings",
                "Storage root must be absolute without parent traversal.",
            ));
        }
        let Some(share) = &self.required_smb_share else {
            return Ok(());
        };
        let expected = expected_share(share)?;
        if !mounted_share(root, &expected)? {
            return Err(unavailable());
        }
        Ok(())
    }
}

fn expected_share(share: &str) -> Result<Url> {
    let parsed = Url::parse(share)
        .map_err(|_| ScannerError::new("invalidSettings", "Invalid SMB share URL."))?;
    if parsed.scheme() != "smb"
        || parsed.host_str().is_none_or(str::is_empty)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path().split('/').filter(|s| !s.is_empty()).count() != 1
    {
        return Err(ScannerError::new(
            "invalidSettings",
            "Expected an SMB share without credentials, such as smb://lake/scans.",
        ));
    }
    Ok(parsed)
}

fn matches_share(source: &str, expected: &Url) -> bool {
    Url::parse(&format!("smb:{source}"))
        .ok()
        .is_some_and(|actual| {
            actual
                .host_str()
                .zip(expected.host_str())
                .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
                && actual.path() == expected.path()
        })
}

fn unavailable() -> ScannerError {
    ScannerError::new(
        "storageUnavailable",
        "The configured SMB share is not mounted at the storage root; local fallback is refused.",
    )
}

#[cfg(target_os = "macos")]
fn mounted_share(root: &Path, expected: &Url) -> Result<bool> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let path = CString::new(root.as_os_str().as_bytes()).map_err(crate::storage)?;
    let mut info = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: path is NUL-terminated, and statfs initializes info only on success.
    if unsafe { libc::statfs(path.as_ptr(), info.as_mut_ptr()) } != 0 {
        return Err(unavailable());
    }
    // SAFETY: the successful statfs call initialized the entire structure.
    let info = unsafe { info.assume_init() };
    let string = |bytes: &[libc::c_char]| {
        String::from_utf8_lossy(
            &bytes
                .iter()
                .take_while(|&&b| b != 0)
                .map(|&b| b as u8)
                .collect::<Vec<_>>(),
        )
        .into_owned()
    };
    Ok(string(&info.f_fstypename) == "smbfs"
        && Path::new(&string(&info.f_mntonname)) == root
        && matches_share(&string(&info.f_mntfromname), expected))
}

#[cfg(target_os = "linux")]
fn mounted_share(root: &Path, expected: &Url) -> Result<bool> {
    let mounts = std::fs::read_to_string("/proc/self/mountinfo").map_err(|_| unavailable())?;
    Ok(linux_mount_matches(&mounts, root, expected))
}

#[cfg(any(target_os = "linux", test))]
fn linux_mount_matches(mounts: &str, root: &Path, expected: &Url) -> bool {
    mounts.lines().any(|line| {
        let Some((left, right)) = line.split_once(" - ") else {
            return false;
        };
        let (Some(mounted_at), Some(kind), Some(source)) = (
            left.split_whitespace().nth(4),
            right.split_whitespace().next(),
            right.split_whitespace().nth(1),
        ) else {
            return false;
        };
        let decode = |value: &str| {
            value
                .replace("\\040", " ")
                .replace("\\011", "\t")
                .replace("\\012", "\n")
                .replace("\\134", "\\")
        };
        matches!(kind, "cifs" | "smb3")
            && Path::new(&decode(mounted_at)) == root
            && matches_share(&decode(source), expected)
    })
}

#[cfg(target_os = "windows")]
fn mounted_share(root: &Path, expected: &Url) -> Result<bool> {
    // UNC paths cannot fall back to a local directory. Mapped drives require a
    // native volume-to-UNC lookup before this policy can accept them.
    let path = root
        .canonicalize()
        .map_err(|_| unavailable())?
        .to_string_lossy()
        .replace('\\', "/");
    let unc = path
        .strip_prefix("//?/UNC/")
        .map(|p| format!("//{p}"))
        .or_else(|| path.starts_with("//").then_some(path.clone()))
        .ok_or_else(unavailable)?;
    Ok(matches_share(&unc, expected))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn mounted_share(_: &Path, _: &Url) -> Result<bool> {
    Err(unavailable())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_local_directory_cannot_substitute_for_a_disconnected_share() {
        let dir = tempfile::tempdir().unwrap();
        let config = StorageConfiguration {
            storage_root: dir.path().to_string_lossy().into(),
            required_smb_share: Some("smb://lake/scans".into()),
        };
        assert!(config.validate().is_err());
        for invalid in [
            "https://lake/scans",
            "smb://user:pass@lake/scans",
            "smb://lake/scans/extra",
            "smb://lake/scans?query=1",
        ] {
            assert!(expected_share(invalid).is_err());
        }
    }
    #[test]
    fn linux_mounts_require_the_right_filesystem_server_share_and_mountpoint() {
        let expected = expected_share("smb://lake/scans").unwrap();
        let mounted = "23 19 0:30 / /mnt/scans rw - cifs //lake/scans rw\n";
        assert!(linux_mount_matches(
            mounted,
            Path::new("/mnt/scans"),
            &expected
        ));
        assert!(!linux_mount_matches(
            mounted,
            Path::new("/mnt/other"),
            &expected
        ));
        assert!(!linux_mount_matches(
            &mounted.replace("cifs", "ext4"),
            Path::new("/mnt/scans"),
            &expected
        ));
        assert!(!linux_mount_matches(
            &mounted.replace("//lake/", "//other/"),
            Path::new("/mnt/scans"),
            &expected
        ));
    }
    #[test]
    fn configured_live_share_can_be_checked_without_writing() {
        let Some(path) = std::env::var_os("SCANNER_COMPAT_STORAGE_CONFIG") else {
            return;
        };
        let config =
            StorageConfiguration::load(Path::new(&path), Path::new("/unavailable")).unwrap();
        config.validate().unwrap();
        crate::StorageRoot::open_configured(config).unwrap();
    }
}
