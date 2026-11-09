//! Scanner I/O only: no database, UI, image recognition or helper processes.
mod backend;
pub use backend::{CaptureEvent, CaptureOutcome, ScannerBackend, persist_settings};
#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(unix)]
pub mod sane;

#[cfg(any(windows, test))]
pub mod twain;

mod native_output;
