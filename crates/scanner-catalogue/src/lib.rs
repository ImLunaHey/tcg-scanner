//! Public catalogue GETs only. No images, OCR text or user inventory are uploaded.
mod cache;
mod client;
mod transport;
mod validation;
pub use client::TcgDex;
pub use transport::JsonTransport;

pub mod library;

pub mod star_wars;
pub mod yugioh;

pub mod persistent;

pub mod reference;
