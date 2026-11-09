//! Version-one wire contracts shared by the service, native UI and web UI.
//! Unknown additive metadata survives a read/write cycle during migration.
pub mod cards;
pub mod device;
pub mod error;
pub mod images;
pub mod scan;

pub type Extensions = std::collections::BTreeMap<String, serde_json::Value>;

pub fn schema_version() -> u32 {
    1
}
