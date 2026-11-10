//! Native process ownership and private local state. No helper processes.
mod credentials;
mod ownership;
mod private;
#[cfg(windows)]
mod windows_acl;
pub use credentials::{create_credential, load_credential};
pub use ownership::HardwareOwner;
pub use private::PrivateDirectory;
use scanner_types::error::{Result, ScannerError};
use std::path::PathBuf;

pub fn application_data() -> Result<PathBuf> {
    dirs::data_local_dir()
        .map(|p| p.join("Scanner"))
        .ok_or_else(|| {
            ScannerError::new(
                "configuration",
                "Cannot locate the user's application data directory.",
            )
        })
}
fn error(message: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("accessDenied", message.to_string())
}
