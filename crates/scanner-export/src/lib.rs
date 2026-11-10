//! Export verified saved images without replacing user files. Browser archives
//! remain bounded; native batch exports stream one image at a time.
mod archive;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;
mod plan;
pub use archive::{archive, image_bytes};
pub use plan::{Batch, Entry};
pub const MEMORY_IMAGE_LIMIT: u64 = 64 * 1024 * 1024;
fn error(message: impl Into<String>) -> scanner_types::error::ScannerError {
    scanner_types::error::ScannerError::new("export", message)
}
