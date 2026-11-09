use crate::{Database, decode, encode, events::append, key, storage};
use rusqlite::{OptionalExtension, Transaction, params};
use scanner_types::{
    error::{Result, ScannerError},
    scan::{ScanRecord, ScanRequest},
};
use uuid::Uuid;

impl Database {
    pub fn reserve_batch_number(&self, id: Uuid) -> Result<u64> {
        self.write(|tx| {
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT number FROM batchNumbers WHERE jobID=?",
                    [key(id)],
                    |r| r.get(0),
                )
                .optional()
                .map_err(storage)?;
            if let Some(number) = existing {
                return u64::try_from(number).map_err(storage);
            }
            tx.execute("INSERT INTO batchNumbers(jobID) VALUES (?)", [key(id)])
                .map_err(storage)?;
            Ok(tx.last_insert_rowid() as u64)
        })
    }
    pub fn insert(&self, record: &ScanRecord, submission: &ScanRequest) -> Result<()> {
        self.write(|tx| {
            tx.execute(
                "INSERT INTO jobs VALUES (?,?,?,?,?)",
                params![
                    key(record.id),
                    encode(submission)?,
                    record.state.as_str(),
                    record.created_at.timestamp_millis() as f64 / 1000.,
                    encode(record)?
                ],
            )
            .map_err(storage)?;
            write_metadata(tx, record)
        })
    }
    pub fn save(&self, record: &ScanRecord) -> Result<()> {
        self.write(|tx| {
            let changed = tx
                .execute(
                    "UPDATE jobs SET state=?,record=? WHERE id=?",
                    params![record.state.as_str(), encode(record)?, key(record.id)],
                )
                .map_err(storage)?;
            if changed != 1 {
                return Err(storage("Job is missing from the database."));
            }
            write_metadata(tx, record)
        })
    }
    pub fn job(&self, id: Uuid) -> Result<Option<ScanRecord>> {
        self.read(|db| {
            db.query_row("SELECT record FROM jobs WHERE id=?", [key(id)], |r| {
                r.get::<_, Vec<u8>>(0)
            })
            .optional()
            .map_err(storage)?
            .map(decode)
            .transpose()
        })
    }
    pub fn submission(&self, id: Uuid) -> Result<Option<ScanRequest>> {
        self.read(|db| {
            db.query_row("SELECT request FROM jobs WHERE id=?", [key(id)], |r| {
                r.get::<_, Vec<u8>>(0)
            })
            .optional()
            .map_err(storage)?
            .map(decode)
            .transpose()
        })
    }
    pub fn jobs(&self, limit: usize, offset: usize) -> Result<Vec<ScanRecord>> {
        if !(1..=101).contains(&limit) || offset > 1_000_000 {
            return Err(ScannerError::new(
                "invalidParams",
                "Invalid history pagination.",
            ));
        }
        self.read(|db| {
            let mut query = db
                .prepare("SELECT record FROM jobs ORDER BY created DESC,id LIMIT ? OFFSET ?")
                .map_err(storage)?;
            query
                .query_map(params![limit as i64, offset as i64], |r| {
                    r.get::<_, Vec<u8>>(0)
                })
                .map_err(storage)?
                .map(|r| decode(r.map_err(storage)?))
                .collect()
        })
    }
    pub fn unfinished(&self) -> Result<Vec<ScanRecord>> {
        self.read(|db| {
            let mut query=db.prepare("SELECT record FROM jobs WHERE state NOT IN ('completed','failed','cancelled') ORDER BY created,id").map_err(storage)?;
            query.query_map([],|r|r.get::<_,Vec<u8>>(0)).map_err(storage)?.map(|r|decode(r.map_err(storage)?)).collect()
        })
    }
    pub fn continuation_batch(&self, id: Uuid) -> Result<Option<Uuid>> {
        self.read(|db| {
            db.query_row(
                "SELECT batchID FROM batchContinuations WHERE requestID=?",
                [key(id)],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(storage)?
            .map(|s| Uuid::parse_str(&s).map_err(storage))
            .transpose()
        })
    }
    pub fn begin_continuation(&self, record: &ScanRecord, request_id: Uuid) -> Result<()> {
        self.write(|tx| {
            tx.execute(
                "INSERT INTO batchContinuations VALUES (?,?)",
                params![key(request_id), key(record.id)],
            )
            .map_err(storage)?;
            if tx
                .execute(
                    "UPDATE jobs SET state=?,record=? WHERE id=? AND state='completed'",
                    params![record.state.as_str(), encode(record)?, key(record.id)],
                )
                .map_err(storage)?
                != 1
            {
                return Err(ScannerError::new(
                    "conflict",
                    "Batch is no longer completed.",
                ));
            }
            write_metadata(tx, record)
        })
    }
}

pub(crate) fn write_metadata(tx: &Transaction<'_>, record: &ScanRecord) -> Result<()> {
    let pending = record
        .extensions
        .get("cardIdentifications")
        .and_then(|v| v.as_array())
        .is_some_and(|xs| xs.iter().any(|x| x["state"] == "processing"));
    let id = key(record.id);
    if pending {
        tx.execute(
            "INSERT OR IGNORE INTO pendingIdentifications VALUES (?)",
            [&id],
        )
        .map_err(storage)?;
    } else {
        tx.execute("DELETE FROM pendingIdentifications WHERE jobID=?", [&id])
            .map_err(storage)?;
    }
    tx.execute(
        "INSERT OR REPLACE INTO batches VALUES (?,?)",
        params![id, encode(record)?],
    )
    .map_err(storage)?;
    for image in &record.images {
        tx.execute(
            "INSERT OR REPLACE INTO images VALUES (?,?,?)",
            params![id, image.sequence, encode(image)?],
        )
        .map_err(storage)?;
    }
    append(
        tx,
        "job.updated",
        &serde_json::json!({"id":record.id,"batchID":record.id,"batchNumber":record.batch_number,"state":record.state,"progress":{"imagesSaved":record.images.len()}}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use scanner_types::{device::Device, scan::ScanState};
    #[test]
    fn reservations_survive_and_retries_do_not_allocate_again() {
        let db = Database::memory().unwrap();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        assert_eq!(db.reserve_batch_number(a).unwrap(), 1);
        assert_eq!(db.reserve_batch_number(a).unwrap(), 1);
        assert_eq!(db.reserve_batch_number(b).unwrap(), 2);
    }
    #[test]
    fn record_and_continuation_updates_are_atomic() {
        let db = Database::memory().unwrap();
        let request = ScanRequest::default();
        let mut record = ScanRecord::new(Device::default(), request.clone(), "B-001".into());
        db.insert(&record, &request).unwrap();
        let continuation = Uuid::new_v4();
        assert!(db.begin_continuation(&record, continuation).is_err());
        assert_eq!(db.continuation_batch(continuation).unwrap(), None);
        record.state = ScanState::Completed;
        db.save(&record).unwrap();
        record.state = ScanState::Queued;
        db.begin_continuation(&record, continuation).unwrap();
        assert_eq!(
            db.continuation_batch(continuation).unwrap(),
            Some(record.id)
        );
        assert_eq!(db.submission(record.id).unwrap(), Some(request));
    }
    #[test]
    fn reopening_keeps_batch_numbers_and_saved_records() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scanner.sqlite");
        let request = ScanRequest::default();
        let record = ScanRecord::new(Device::default(), request.clone(), "B-001".into());
        {
            let db = Database::open(&path).unwrap();
            assert_eq!(db.reserve_batch_number(record.id).unwrap(), 1);
            db.insert(&record, &request).unwrap();
        }
        let db = Database::open(&path).unwrap();
        assert_eq!(db.job(record.id).unwrap().unwrap().id, record.id);
        assert_eq!(db.reserve_batch_number(Uuid::new_v4()).unwrap(), 2);
    }
    #[test]
    fn existing_swift_ledger_is_readable_without_migration() {
        let Some(path) = std::env::var_os("SCANNER_COMPAT_DATABASE") else {
            return;
        };
        let db = Database::read_only(std::path::Path::new(&path)).unwrap();
        let records = db.jobs(100, 0).unwrap();
        assert!(!records.is_empty());
        for record in records {
            assert_eq!(db.job(record.id).unwrap().unwrap().id, record.id);
        }
    }
}
