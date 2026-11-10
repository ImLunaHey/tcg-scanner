//! Transactional SQLite persistence. No hardware, networking or UI dependencies.
mod cache;
mod catalog;
mod events;
mod jobs;
mod paths;
mod profiles;
mod recovery;
mod schema;

pub use events::{EventPage, ServiceEvent};
pub use recovery::mark_interrupted;
use rusqlite::{Connection, OpenFlags, Transaction};
use scanner_types::error::{Result, ScannerError};
use std::{path::Path, sync::Mutex};

pub struct Database {
    connection: Mutex<Connection>,
}

fn storage(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("storage", error.to_string())
}

pub(crate) fn key(id: uuid::Uuid) -> String {
    id.to_string().to_uppercase()
}
pub(crate) fn encode(value: &impl serde::Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(storage)
}
pub(crate) fn decode<T: serde::de::DeserializeOwned>(bytes: Vec<u8>) -> Result<T> {
    serde_json::from_slice(&bytes).map_err(storage)
}

impl Database {
    pub fn memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory().map_err(storage)?)
    }
    pub fn open(path: &Path) -> Result<Self> {
        paths::prepare(path)?;
        Self::initialize(Connection::open(path).map_err(storage)?)
    }
    /// Does not migrate, recover, change pragmas on disk or write the live ledger.
    pub fn read_only(path: &Path) -> Result<Self> {
        let connection =
            Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(storage)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
    fn initialize(mut connection: Connection) -> Result<Self> {
        connection
            .busy_timeout(std::time::Duration::from_secs(3))
            .map_err(storage)?;
        connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;")
            .map_err(storage)?;
        schema::migrate(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
    pub(crate) fn read<T>(&self, work: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| storage("Database lock poisoned"))?;
        work(&connection)
    }
    pub(crate) fn write<T>(&self, work: impl FnOnce(&Transaction<'_>) -> Result<T>) -> Result<T> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| storage("Database lock poisoned"))?;
        let transaction = connection.transaction().map_err(storage)?;
        let result = work(&transaction)?;
        transaction.commit().map_err(storage)?;
        Ok(result)
    }
}

mod inventory;
