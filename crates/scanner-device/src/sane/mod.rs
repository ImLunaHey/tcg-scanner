//! SANE 1 scanner access through its native C ABI, without `scanimage` or helpers.
mod acquisition;
mod bridge;
mod cancellation;
mod configuration;
mod discovery;
mod ffi;
mod frames;
mod options;
mod session;
mod value;
pub use bridge::SaneBackend;
