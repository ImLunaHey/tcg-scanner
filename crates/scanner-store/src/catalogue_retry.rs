use crate::{Database, decode, storage};
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use scanner_types::{error::Result, scan::ScanRecord};
impl Database {
    /// Select one due terminal job using the existing v1 JSON deadlines. Date
    /// arithmetic handles both Swift's Z and Rust's offset/fractional timestamps.
    pub fn next_catalogue_retry(&self, now: DateTime<Utc>) -> Result<Option<ScanRecord>> {
        self.read(|db| {
            db.query_row("SELECT jobs.record FROM jobs, json_each(CAST(jobs.record AS TEXT), '$.cardIdentifications') AS item
                WHERE jobs.state IN ('completed','failed','cancelled')
                  AND json_extract(item.value, '$.state') IN ('awaitingCatalogue','failed')
                  AND julianday(json_extract(item.value, '$.catalogueRetryAfter')) <= julianday(?)
                ORDER BY julianday(json_extract(item.value, '$.catalogueRetryAfter')), jobs.id LIMIT 1", [now.to_rfc3339()], |row| row.get::<_,Vec<u8>>(0))
                .optional().map_err(storage)?.map(decode).transpose()
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use scanner_types::{
        device::Device,
        identification::{CardIdentification, IdentificationRequest, IdentificationState},
        scan::{ScanRequest, ScanState},
    };
    use uuid::Uuid;
    fn record(state: IdentificationState, due: Option<&str>) -> ScanRecord {
        let request = ScanRequest::default();
        let mut record = ScanRecord::new(Device::default(), request, "/unused".into());
        record.state = ScanState::Completed;
        let mut identification = CardIdentification::pending(
            IdentificationRequest {
                id: Uuid::new_v4(),
                job_id: record.id,
                sequence: 1,
                language: Default::default(),
                extensions: Default::default(),
            },
            "a".repeat(64),
            None,
        );
        identification.state = state;
        identification.catalogue_retry_after =
            due.map(|v| DateTime::parse_from_rfc3339(v).unwrap().with_timezone(&Utc));
        record.replace_identification(&identification).unwrap();
        if let Some(date) = due {
            record.extensions.get_mut("cardIdentifications").unwrap()[0]["catalogueRetryAfter"] =
                serde_json::json!(date);
        }
        record
    }
    #[test]
    fn selects_oldest_due_retry_and_excludes_reviewed_or_unfinished_work() {
        let database = Database::memory().unwrap();
        let now = DateTime::parse_from_rfc3339("2026-11-10T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let later = record(
            IdentificationState::AwaitingCatalogue,
            Some("2026-11-10T11:00:00Z"),
        );
        let due = record(
            IdentificationState::Failed,
            Some("2026-11-10T10:30:00+01:00"),
        );
        let old = record(
            IdentificationState::AwaitingCatalogue,
            Some("2026-11-09T10:00:00Z"),
        );
        let reviewed = record(IdentificationState::Confirmed, Some("2026-11-01T10:00:00Z"));
        let mut active = record(IdentificationState::Failed, Some("2026-11-01T10:00:00Z"));
        active.state = ScanState::Processing;
        let unrequested = record(IdentificationState::Failed, None);
        for record in [&later, &due, &old, &reviewed, &active, &unrequested] {
            database.insert(record, &record.request).unwrap();
        }
        assert_eq!(
            database.next_catalogue_retry(now).unwrap().unwrap().id,
            old.id
        );
        let mut cleared = old.clone();
        let mut identification = cleared.identifications().unwrap().remove(0);
        identification.catalogue_retry_after = None;
        cleared.replace_identification(&identification).unwrap();
        database.save(&cleared).unwrap();
        assert_eq!(
            database.next_catalogue_retry(now).unwrap().unwrap().id,
            due.id
        );
    }
}
