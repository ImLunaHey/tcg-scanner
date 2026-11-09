//! Hand-written Rust ABI subset translated from sane/sane.h (public domain).
//! Original header: Copyright (C) 1997–1999 David Mosberger-Tang and Andreas Beck.
//! The header permits use/modification with its copyright and modification notice.
//! Modification: Rust repr(C) declarations and dynamically loaded function pointers.
use libloading::Library;
use scanner_types::error::{Result, ScannerError};
use std::{
    ffi::{CStr, c_char, c_int, c_uchar, c_void},
    path::Path,
};
pub type Handle = *mut c_void;
pub type Status = c_int;
pub type Cancel = unsafe extern "C" fn(Handle);
#[repr(C)]
pub struct Device {
    pub name: *const c_char,
    pub vendor: *const c_char,
    pub model: *const c_char,
    pub kind: *const c_char,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Range {
    pub min: c_int,
    pub max: c_int,
    pub quant: c_int,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union Constraint {
    pub strings: *const *const c_char,
    pub words: *const c_int,
    pub range: *const Range,
}
#[repr(C)]
pub struct Descriptor {
    pub name: *const c_char,
    pub title: *const c_char,
    pub description: *const c_char,
    pub kind: c_int,
    pub unit: c_int,
    pub size: c_int,
    pub capabilities: c_int,
    pub constraint_kind: c_int,
    pub constraint: Constraint,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Parameters {
    pub format: c_int,
    pub last_frame: c_int,
    pub bytes_per_line: c_int,
    pub pixels_per_line: c_int,
    pub lines: c_int,
    pub depth: c_int,
}
type Auth = unsafe extern "C" fn(*const c_char, *mut c_char, *mut c_char);
pub struct Api {
    _library: Library,
    initialized: bool,
    pub exit: unsafe extern "C" fn(),
    pub devices: unsafe extern "C" fn(*mut *const *const Device, c_int) -> Status,
    pub open: unsafe extern "C" fn(*const c_char, *mut Handle) -> Status,
    pub close: unsafe extern "C" fn(Handle),
    pub descriptor: unsafe extern "C" fn(Handle, c_int) -> *const Descriptor,
    pub control: unsafe extern "C" fn(Handle, c_int, c_int, *mut c_void, *mut c_int) -> Status,
    pub parameters: unsafe extern "C" fn(Handle, *mut Parameters) -> Status,
    pub start: unsafe extern "C" fn(Handle) -> Status,
    pub read: unsafe extern "C" fn(Handle, *mut c_uchar, c_int, *mut c_int) -> Status,
    pub cancel: Cancel,
    pub version: c_int,
}
impl Api {
    pub fn load(path: &Path) -> Result<Self> {
        // SAFETY: trusted application-selected SANE 1 library; signatures match
        // sane.h. The worker retains the library through every handle/cancel call.
        unsafe {
            let library = Library::new(path).map_err(error)?;
            let init: unsafe extern "C" fn(*mut c_int, Option<Auth>) -> Status =
                *library.get(b"sane_init\0").map_err(error)?;
            let mut api = Self {
                exit: *library.get(b"sane_exit\0").map_err(error)?,
                devices: *library.get(b"sane_get_devices\0").map_err(error)?,
                open: *library.get(b"sane_open\0").map_err(error)?,
                close: *library.get(b"sane_close\0").map_err(error)?,
                descriptor: *library
                    .get(b"sane_get_option_descriptor\0")
                    .map_err(error)?,
                control: *library.get(b"sane_control_option\0").map_err(error)?,
                parameters: *library.get(b"sane_get_parameters\0").map_err(error)?,
                start: *library.get(b"sane_start\0").map_err(error)?,
                read: *library.get(b"sane_read\0").map_err(error)?,
                cancel: *library.get(b"sane_cancel\0").map_err(error)?,
                _library: library,
                initialized: false,
                version: 0,
            };
            check(init(&mut api.version, None), "initialize SANE")?;
            api.initialized = true;
            if (api.version >> 24) & 0xff != 1 {
                return Err(error("Only SANE major version 1 is supported."));
            }
            Ok(api)
        }
    }
}
pub fn check(status: Status, operation: &str) -> Result<()> {
    if status == 0 {
        return Ok(());
    }
    let (kind, message) = match status {
        1 => ("unsupported", "operation unsupported"),
        2 => ("cancelled", "operation cancelled"),
        3 => ("busy", "device busy"),
        4 => ("invalidSettings", "invalid argument or missing device"),
        5 => ("driver", "unexpected end of frame"),
        6 => ("paperJam", "paper jam"),
        7 => ("feederEmpty", "feeder empty"),
        8 => ("coverOpen", "cover open"),
        9 => ("driver", "device I/O error"),
        10 => ("resourceLimit", "insufficient memory"),
        11 => ("accessDenied", "device access denied"),
        _ => ("driver", "unknown status"),
    };
    Err(ScannerError {
        kind: kind.into(),
        message: format!("Could not {operation}: {message}."),
        domain: Some("SANE".into()),
        code: Some(status.into()),
    })
}
pub fn error(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("driver", error.to_string())
}
/// Caller guarantees the SANE API supplied a live NUL-terminated string.
pub unsafe fn string(value: *const c_char) -> Result<String> {
    if value.is_null() {
        return Ok(String::new());
    }
    let bytes = unsafe { CStr::from_ptr(value) }.to_bytes();
    if bytes.len() > 4096 {
        return Err(error("Oversized SANE string."));
    }
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

impl Drop for Api {
    fn drop(&mut self) {
        // SAFETY: worker drops every Session before this API; cancellation has
        // been disarmed. The native library remains loaded through sane_exit.
        if self.initialized {
            unsafe {
                (self.exit)();
            }
        }
    }
}
