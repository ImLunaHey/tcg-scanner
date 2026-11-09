//! Windows TWAIN 2 adapter. Portable ABI/container tests also run on Unix.
#[cfg_attr(not(windows), allow(dead_code))]
mod abi;
#[cfg(windows)]
mod bridge;
#[cfg(any(windows, test))]
mod capabilities;
#[cfg_attr(not(windows), allow(dead_code))]
mod container;
#[cfg(any(windows, test))]
mod inspection;
#[cfg(any(windows, test))]
mod manager;
#[cfg(windows)]
mod window;
#[cfg(windows)]
pub use bridge::TwainBackend;
