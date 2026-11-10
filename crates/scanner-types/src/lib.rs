//! Version-one wire contracts shared by the service, native UI and web UI.
//! Unknown additive metadata survives a read/write cycle during migration.
pub mod cards;
pub mod catalogue;
pub mod device;
pub mod error;
pub mod images;
pub mod scan;
pub mod service;

pub type Extensions = std::collections::BTreeMap<String, serde_json::Value>;

pub fn schema_version() -> u32 {
    1
}
pub mod image_access;

pub mod identification;
pub mod recognition;
