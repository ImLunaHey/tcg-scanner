use crate::storage;
use rusqlite::Connection;
use scanner_types::error::Result;

pub fn migrate(connection: &mut Connection) -> Result<()> {
    let tx = connection.transaction().map_err(storage)?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS grdb_migrations(identifier TEXT NOT NULL PRIMARY KEY);
        CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY,request BLOB NOT NULL,state TEXT NOT NULL,created REAL NOT NULL,record BLOB NOT NULL);
        CREATE INDEX IF NOT EXISTS jobs_history ON jobs(created DESC,id);
        CREATE TABLE IF NOT EXISTS batches(id TEXT PRIMARY KEY REFERENCES jobs(id),manifest BLOB NOT NULL);
        CREATE TABLE IF NOT EXISTS images(jobID TEXT NOT NULL REFERENCES jobs(id),sequence INTEGER NOT NULL,record BLOB NOT NULL,PRIMARY KEY(jobID,sequence));
        CREATE TABLE IF NOT EXISTS profiles(id TEXT PRIMARY KEY,record BLOB NOT NULL);
        CREATE TABLE IF NOT EXISTS devices(id TEXT PRIMARY KEY,record BLOB NOT NULL,connected INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS capabilities(deviceID TEXT PRIMARY KEY,record BLOB NOT NULL);
        CREATE TABLE IF NOT EXISTS events(cursor INTEGER PRIMARY KEY AUTOINCREMENT,timestamp REAL NOT NULL,kind TEXT NOT NULL,payload BLOB NOT NULL);
        CREATE TABLE IF NOT EXISTS deviceSettings(deviceID TEXT PRIMARY KEY,record BLOB NOT NULL);
        CREATE TABLE IF NOT EXISTS batchNumbers(number INTEGER PRIMARY KEY AUTOINCREMENT,jobID TEXT NOT NULL UNIQUE);
        INSERT INTO batchNumbers(jobID) SELECT id FROM jobs WHERE NOT EXISTS (SELECT 1 FROM batchNumbers WHERE jobID=jobs.id) ORDER BY created,id;
        CREATE TABLE IF NOT EXISTS pendingIdentifications(jobID TEXT PRIMARY KEY REFERENCES jobs(id));
        CREATE TABLE IF NOT EXISTS marketPriceCache(key TEXT PRIMARY KEY,fetched REAL NOT NULL,record BLOB NOT NULL);
        CREATE TABLE IF NOT EXISTS storeTransfers(jobID TEXT PRIMARY KEY REFERENCES jobs(id),state TEXT NOT NULL,record BLOB NOT NULL);
        CREATE TABLE IF NOT EXISTS batchContinuations(requestID TEXT PRIMARY KEY,batchID TEXT NOT NULL REFERENCES jobs(id));").map_err(storage)?;
    for name in [
        "v1-job-ledger",
        "v2-device-settings",
        "v3-batch-numbers",
        "v4-identification-recovery",
        "v5-market-price-cache",
        "v6-store-transfers",
        "v7-batch-continuations",
    ] {
        tx.execute("INSERT OR IGNORE INTO grdb_migrations VALUES (?)", [name])
            .map_err(storage)?;
    }
    tx.commit().map_err(storage)
}
