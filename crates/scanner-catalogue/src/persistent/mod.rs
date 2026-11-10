//! Optional host-configured, bounded cache of validated public catalogue data.
mod database;
use crate::JsonTransport;
use scanner_types::error::{Result, ScannerError};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Semaphore;
pub struct Documents {
    connection: Mutex<rusqlite::Connection>,
    workers: Arc<Semaphore>,
}
impl Documents {
    /// Opens a private SQLite cache. No inventory, scan pixels or credentials are
    /// stored here. Maximum live payload is 256 MiB, 512 entries, 64 MiB each.
    pub fn open(path: &Path) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            connection: Mutex::new(database::open(path)?),
            workers: Arc::new(Semaphore::new(4)),
        }))
    }
    pub async fn get(
        self: &Arc<Self>,
        key: &str,
        ttl: Duration,
        limit: usize,
    ) -> Result<Option<Vec<u8>>> {
        let key = key.to_owned();
        self.work(move |db| database::get(db, &key, ttl, limit))
            .await
    }
    pub async fn put(self: &Arc<Self>, key: &str, body: Vec<u8>) -> Result<()> {
        let key = key.to_owned();
        self.work(move |db| database::put(db, &key, &body)).await
    }
    async fn work<T: Send + 'static>(
        self: &Arc<Self>,
        work: impl FnOnce(&mut rusqlite::Connection) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let permit = self.workers.clone().try_acquire_owned().map_err(error)?;
        let cache = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut db = cache.connection.lock().map_err(error)?;
            work(&mut db)
        })
        .await
        .map_err(error)?
    }
}
/// Validate before persistence and after loading. Malformed cached metadata is a
/// miss; cache I/O failure never discards a successful network/catalogue result.
pub(crate) async fn load<T, F>(
    transport: &dyn JsonTransport,
    cache: Option<&Arc<Documents>>,
    key: &str,
    path: &str,
    limit: usize,
    ttl: Duration,
    decode: F,
) -> Result<T>
where
    T: Send + 'static,
    F: Fn(&[u8]) -> Result<T> + Send + Sync + 'static,
{
    let decode = Arc::new(decode);
    if let Some(cache) = cache
        && let Ok(Some(bytes)) = cache.get(key, ttl, limit).await
    {
        let parser = decode.clone();
        if let Ok(value) = tokio::task::spawn_blocking(move || parser(&bytes))
            .await
            .map_err(error)?
        {
            return Ok(value);
        }
    }
    let bytes = transport.get(path, limit).await?;
    if bytes.len() > limit {
        return Err(error("Catalogue exceeds response bound."));
    }
    let (value, bytes) =
        tokio::task::spawn_blocking(move || decode(&bytes).map(|value| (value, bytes)))
            .await
            .map_err(error)??;
    if let Some(cache) = cache {
        let _ = cache.put(key, bytes).await;
    }
    Ok(value)
}
fn error(e: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("catalogueCache", e.to_string())
}
