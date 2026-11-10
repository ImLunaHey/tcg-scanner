//! Pure inventory projection. Reads supplied records, never scans, edits files,
//! or performs network requests. Catalogue adapters remain outside this crate.
mod builder;
mod entry;
mod names;
mod page;
pub use builder::Inventory;
pub use entry::selected_candidate;
