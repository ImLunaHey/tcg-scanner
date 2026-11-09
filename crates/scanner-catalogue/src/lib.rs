//! Public catalogue GETs only. No images, OCR text or user inventory are uploaded.
mod cache;
mod client;
mod transport;
mod validation;
pub use client::TcgDex;
pub use transport::JsonTransport;
