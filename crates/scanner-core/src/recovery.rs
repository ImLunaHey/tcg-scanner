use scanner_files::StorageRoot;
use scanner_store::{Database, mark_interrupted};
use scanner_types::{
    error::{Result, ScannerError},
    scan::ScanRecord,
};
use serde_json::Value;

/// Run under exclusive process ownership, before constructing an Engine or
/// accepting clients. Never feeds hardware, resumes work or changes image files.
/// With unavailable storage, retain ledger metadata and record explicit warnings.
pub fn recover_before_start(database: &Database, storage: Option<&StorageRoot>) -> Result<usize> {
    crate::staging::recover(database)?;
    let records = database.recovery_records()?;
    for mut record in records.iter().cloned() {
        let mut publish = true;
        if let Some(root) = storage {
            if !record.state.terminal() {
                match reconcile(root, &record) {
                    Ok(disk) => record = disk,
                    Err(error) => {
                        publish = false;
                        record.warnings.push(format!(
                            "Disk manifest was not adopted or overwritten during recovery: {}",
                            error.message
                        ));
                    }
                }
            }
        } else {
            record.warnings.push("Scan storage was unavailable during recovery. Files were not read or changed; inspect retained files after reconnecting storage.".into());
        }
        if !record.state.terminal() {
            record.warnings.push("Unindexed or partial files may remain in the batch directory or staging directories. Recovery never deletes or overwrites image files.".into());
        }
        mark_interrupted(&mut record)?;
        if publish
            && let Some(root) = storage
            && let Err(error) = root.save_manifest(&record)
        {
            record.warnings.push(format!(
                "Could not update recovered manifest: {}",
                error.message
            ));
        }
        database.save(&record)?;
    }
    Ok(records.len())
}
fn reconcile(root: &StorageRoot, ledger: &ScanRecord) -> Result<ScanRecord> {
    let disk = root.read_manifest(ledger)?;
    if disk.schema_version != 1
        || disk.id != ledger.id
        || disk.request != ledger.request
        || disk.output_directory != ledger.output_directory
        || disk.created_at != ledger.created_at
        || disk.batch_number != ledger.batch_number
        || disk.device.id != ledger.device.id
        || last_continuation(&disk) != last_continuation(ledger)
        || disk.images.len() < ledger.images.len()
        || disk.images.len() > 1000
        || disk
            .images
            .iter()
            .enumerate()
            .any(|(i, image)| image.sequence != i as u32 + 1)
    {
        return Err(mismatch());
    }
    // A newer manifest can append originals, but cannot replace ledger originals,
    // immutable edits, or reviewed identities already committed to SQLite.
    for (old, new) in ledger.images.iter().zip(&disk.images) {
        if serde_json::to_value(old).ok() != serde_json::to_value(new).ok() {
            return Err(mismatch());
        }
    }
    let edits = disk.processed_images.as_deref().unwrap_or_default();
    if edits.len() > 8000 {
        return Err(mismatch());
    }
    for old in ledger.processed_images.as_deref().unwrap_or_default() {
        if !edits
            .iter()
            .any(|new| serde_json::to_value(old).ok() == serde_json::to_value(new).ok())
        {
            return Err(mismatch());
        }
    }
    preserve_reviews(ledger, &disk)?;
    for image in disk.images.iter().chain(edits.iter().map(|e| &e.image)) {
        root.verify_image(&disk, image)?;
    }
    Ok(disk)
}
fn last_continuation(record: &ScanRecord) -> Option<&Value> {
    record
        .extensions
        .get("continuations")
        .and_then(Value::as_array)
        .and_then(|v| v.last())
        .and_then(|v| v.get("requestID"))
}
fn preserve_reviews(ledger: &ScanRecord, disk: &ScanRecord) -> Result<()> {
    let entries = |record: &ScanRecord| {
        record
            .extensions
            .get("cardIdentifications")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let disk_entries = entries(disk);
    for old in entries(ledger).iter().filter(|v| {
        matches!(v["state"].as_str(), Some("confirmed" | "rejected")) || v.get("review").is_some()
    }) {
        if !disk_entries.contains(old) {
            return Err(mismatch());
        }
    }
    Ok(())
}
fn mismatch() -> ScannerError {
    ScannerError::new(
        "recoveryMismatch",
        "Manifest identity, immutable history or reviewed metadata does not match the ledger.",
    )
}
