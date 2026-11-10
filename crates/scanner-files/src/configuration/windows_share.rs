use super::unavailable;
use scanner_types::error::Result;
use std::{os::windows::ffi::OsStrExt, path::Path};
use windows_sys::Win32::NetworkManagement::WNet::{
    UNIVERSAL_NAME_INFO_LEVEL, UNIVERSAL_NAME_INFOW, WNetGetUniversalNameW,
};

// Resolve an existing, canonical path. WNet only queries a current mapping; it
// cannot reconnect a drive or display a credential prompt.
pub(super) fn resolve(path: &Path) -> Result<String> {
    let value = path.to_str().ok_or_else(unavailable)?;
    if let Some(unc) = unc(value) {
        return Ok(unc);
    }
    let drive = value.strip_prefix(r"\\?\").unwrap_or(value);
    let wide: Vec<u16> = std::ffi::OsStr::new(drive)
        .encode_wide()
        .chain(Some(0))
        .collect();
    // Fixed bounded, pointer-aligned storage includes the structure and string.
    let mut buffer = vec![0usize; 65536 / std::mem::size_of::<usize>()];
    let mut size = 65536u32;
    // SAFETY: input is terminated; output is writable/aligned for the structure.
    let code = unsafe {
        WNetGetUniversalNameW(
            wide.as_ptr(),
            UNIVERSAL_NAME_INFO_LEVEL,
            buffer.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if code != 0 || size > 65536 {
        return Err(unavailable());
    }
    // SAFETY: a successful call initialized UNIVERSAL_NAME_INFOW in the buffer.
    let info = unsafe { &*buffer.as_ptr().cast::<UNIVERSAL_NAME_INFOW>() };
    let start = buffer.as_ptr() as usize;
    let pointer = info.lpUniversalName as usize;
    let end = start + 65536;
    if pointer < start + std::mem::size_of::<UNIVERSAL_NAME_INFOW>()
        || pointer >= end
        || !pointer.is_multiple_of(2)
    {
        return Err(unavailable());
    }
    // SAFETY: range and alignment were checked inside the live allocation.
    let string = unsafe { std::slice::from_raw_parts(info.lpUniversalName, (end - pointer) / 2) };
    let length = string
        .iter()
        .position(|c| *c == 0)
        .ok_or_else(unavailable)?;
    let value = String::from_utf16(&string[..length]).map_err(|_| unavailable())?;
    unc(&value).ok_or_else(unavailable)
}
fn unc(value: &str) -> Option<String> {
    let path = value.replace('\\', "/");
    if let Some(rest) = path.strip_prefix("//?/UNC/") {
        Some(format!("//{}", rest.trim_end_matches('/')))
    } else if path.starts_with("//") && !path.starts_with("//?/") && !path.starts_with("//./") {
        Some(path.trim_end_matches('/').into())
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_unc_forms_but_never_local_or_device_paths() {
        for value in [r"\\lake\scans", r"\\?\UNC\lake\scans\"] {
            assert_eq!(unc(value).as_deref(), Some("//lake/scans"));
        }
        for value in [r"C:\scans", r"\\?\C:\scans", r"\\.\C:\scans"] {
            assert!(unc(value).is_none());
        }
        let temp = tempfile::tempdir().unwrap();
        assert!(resolve(&temp.path().canonicalize().unwrap()).is_err());
    }
}
