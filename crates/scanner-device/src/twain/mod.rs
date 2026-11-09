//! Windows TWAIN 2 adapter. Portable ABI/container tests also run on Unix.
mod abi;
#[cfg(windows)]
mod bridge;
#[cfg(any(windows, test))]
mod capabilities;
mod container;
#[cfg(any(windows, test))]
mod inspection;
#[cfg(any(windows, test))]
mod manager;
#[cfg(windows)]
mod window;
#[cfg(windows)]
pub use bridge::TwainBackend;
mod acquisition;
mod configuration;
mod frame;
mod phase;
mod session;
mod transfer_abi;
