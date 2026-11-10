use crate::configuration::config;
use scanner_types::error::Result;
use std::{os::windows::ffi::OsStrExt, path::Path};
use windows_sys::Win32::System::Registry::*;
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
pub(super) fn set(binary: &Path, source: &Path, enable: bool) -> Result<()> {
    // Windows paths cannot contain quotes. Preserve UTF-16, including paths that
    // cannot be represented as UTF-8, and quote both complete arguments.
    let command = command(binary, source);
    if command.len() > 32768 {
        return Err(config("Startup command exceeds Windows limit."));
    }
    let key = open_key()?;
    let name = wide("ScannerRust");
    let mut existing = vec![0u16; 32768];
    let mut size = 65536u32;
    let mut kind = 0;
    // SAFETY: buffer and size match; all pointers live through this synchronous call.
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            name.as_ptr(),
            std::ptr::null(),
            &mut kind,
            existing.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if enable {
        if status != windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND {
            return Err(config(
                "Startup registration already exists or cannot be read; retained.",
            ));
        }
        // SAFETY: command is a terminated UTF-16 buffer with the specified length.
        let status = unsafe {
            RegSetValueExW(
                key.0,
                name.as_ptr(),
                0,
                REG_SZ,
                command.as_ptr().cast(),
                (command.len() * 2) as u32,
            )
        };
        if status != 0 {
            return Err(config(format!(
                "Cannot save startup registration ({status})."
            )));
        }
    } else {
        if status != 0
            || kind != REG_SZ
            || size as usize != command.len() * 2
            || existing[..command.len()] != command
        {
            return Err(config(
                "Startup entry belongs to a different installation or cannot be read; retained.",
            ));
        }
        // SAFETY: key is open for writing and name is terminated.
        let status = unsafe { RegDeleteValueW(key.0, name.as_ptr()) };
        if status != 0 {
            return Err(config(format!(
                "Cannot remove startup registration ({status})."
            )));
        }
    }
    Ok(())
}

fn command(binary: &Path, source: &Path) -> Vec<u16> {
    let mut command = wide("\"");
    command.pop();
    command.extend(binary.as_os_str().encode_wide());
    command.extend("\" serve --config \"".encode_utf16());
    command.extend(source.as_os_str().encode_wide());
    command.extend([b'"' as u16, 0]);
    command
}

fn open_key() -> Result<Key> {
    let subkey = wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let mut key = std::ptr::null_mut();
    // SAFETY: terminated strings and writable output; current-user registry only.
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(config(format!(
            "Cannot open startup registration ({status})."
        )));
    }
    Ok(Key(key))
}
