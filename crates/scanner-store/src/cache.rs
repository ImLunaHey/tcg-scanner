use crate::{Database, decode, encode, events::append, key, storage};
use rusqlite::{OptionalExtension, params};
use scanner_types::error::Result;
use serde_json::Value;
use uuid::Uuid;

impl Database {
    pub fn cached_value(&self, key: &str) -> Result<Option<Value>> {
        self.read(|db| {
            db.query_row(
                "SELECT record FROM marketPriceCache WHERE key=? AND length(record)<=1048576",
                [key],
                |r| r.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(storage)?
            .map(decode)
            .transpose()
        })
    }
    pub fn cache_value(&self, key: &str, value: &Value) -> Result<()> {
        let encoded = encode(value)?;
        if key.is_empty() || key.len() > 200 || encoded.len() > 1024 * 1024 {
            return Err(storage("Price cache entry exceeds bounds."));
        }
        self.write(|tx|{tx.execute("INSERT OR REPLACE INTO marketPriceCache VALUES (?,?,?)",params![key,chrono::Utc::now().timestamp_millis() as f64/1000.,encoded]).map_err(storage)?;
            tx.execute("DELETE FROM marketPriceCache WHERE key IN (SELECT key FROM marketPriceCache ORDER BY fetched DESC LIMIT -1 OFFSET 5000)",[]).map_err(storage)?;Ok(())})
    }
    pub fn store_transfer(&self, id: Uuid) -> Result<Option<Value>> {
        self.read(|db| {
            db.query_row(
                "SELECT record FROM storeTransfers WHERE jobID=?",
                [key(id)],
                |r| r.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(storage)?
            .map(decode)
            .transpose()
        })
    }
    pub fn save_store_transfer(
        &self,
        id: Uuid,
        state: &str,
        record: &Value,
        status: &Value,
    ) -> Result<()> {
        self.write(|tx|{tx.execute("INSERT INTO storeTransfers VALUES (?,?,?) ON CONFLICT(jobID) DO UPDATE SET state=excluded.state,record=excluded.record",params![key(id),state,encode(record)?]).map_err(storage)?;
            append(tx,"store.transfer.updated",status)})
    }
    pub fn interrupted_store_transfers(&self) -> Result<Vec<Value>> {
        self.read(|db| {
            let mut q = db
                .prepare("SELECT record FROM storeTransfers WHERE state='sending'")
                .map_err(storage)?;
            q.query_map([], |r| r.get::<_, Vec<u8>>(0))
                .map_err(storage)?
                .map(|r| decode(r.map_err(storage)?))
                .collect()
        })
    }
}
