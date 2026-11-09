use crate::{Database, decode, storage};
use scanner_types::{
    error::{Result, ScannerError},
    scan::{ScanRecord, ScanState},
};

impl Database {
    /// Caller must hold the hardware-owner lock. Never feeds or deletes files.
    /// Disk reconciliation belongs to the storage layer before this transition.
    pub fn recover_interrupted(&self) -> Result<usize> {
        let records:Vec<ScanRecord>=self.read(|db|{
            let mut q=db.prepare("SELECT record FROM jobs WHERE state NOT IN ('completed','failed','cancelled') OR id IN (SELECT jobID FROM pendingIdentifications)").map_err(storage)?;
            q.query_map([],|r|r.get::<_,Vec<u8>>(0)).map_err(storage)?.map(|r|decode(r.map_err(storage)?)).collect()
        })?;
        for mut record in records.iter().cloned() {
            if !record.state.terminal() {
                record.state = ScanState::Failed;
                record.completed_at = Some(chrono::Utc::now());
                record.session_closed = false;
                record.errors.push(ScannerError::new("interrupted","Service stopped before this job finished. Physical feeding was not resumed; retained images are unchanged."));
            }
            if let Some(items) = record
                .extensions
                .get_mut("cardIdentifications")
                .and_then(|v| v.as_array_mut())
            {
                for item in items.iter_mut().filter(|i| i["state"] == "processing") {
                    item["state"] = "failed".into();
                    item["completedAt"] = chrono::Utc::now().to_rfc3339().into();
                    item["error"] = serde_json::json!({"kind":"interrupted","message":"Identification was interrupted. Retry explicitly."});
                }
            }
            if let Some(item) = record
                .extensions
                .get_mut("continuations")
                .and_then(|v| v.as_array_mut())
                .and_then(|v| v.last_mut())
                && !matches!(
                    item["state"].as_str(),
                    Some("completed" | "failed" | "cancelled")
                )
            {
                item["state"] = record.state.as_str().into();
                item["completedAt"] = serde_json::to_value(record.completed_at).map_err(storage)?;
                let first = item["firstSequence"].as_u64().unwrap_or(1);
                item["imagesSaved"] = record
                    .images
                    .len()
                    .saturating_sub(first.saturating_sub(1) as usize)
                    .into();
            }
            self.save(&record)?;
        }
        Ok(records.len())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use scanner_types::{device::Device, scan::ScanRequest};
    #[test]
    fn recovery_fails_inflight_work_without_resuming_it() {
        let db = Database::memory().unwrap();
        let request = ScanRequest::default();
        let mut record = ScanRecord::new(Device::default(), request.clone(), "B-001".into());
        record.state = ScanState::Scanning;
        record.extensions.insert("cardIdentifications".into(),serde_json::json!([{"state":"processing"},{"state":"confirmed","selectedCardID":"sv09-120"}]));
        db.insert(&record, &request).unwrap();
        assert_eq!(db.recover_interrupted().unwrap(), 1);
        let restored = db.job(record.id).unwrap().unwrap();
        assert_eq!(restored.state, ScanState::Failed);
        assert!(!restored.session_closed);
        assert_eq!(
            restored.extensions["cardIdentifications"][0]["state"],
            "failed"
        );
        assert_eq!(
            restored.extensions["cardIdentifications"][1]["selectedCardID"],
            "sv09-120"
        );
        assert_eq!(db.recover_interrupted().unwrap(), 0);
    }
}
