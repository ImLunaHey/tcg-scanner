use super::error;
use rusqlite::{Connection, OptionalExtension, params};
use scanner_types::error::Result;
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};
const PAYLOAD_LIMIT: usize = 256 * 1024 * 1024;
pub(super) fn open(path: &Path) -> Result<Connection> {
    let parent = path
        .parent()
        .ok_or_else(|| error("Missing cache parent."))?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| error("Invalid cache filename."))?;
    let directory = scanner_platform::PrivateDirectory::open(parent, true)?;
    for suffix in ["", "-wal", "-shm", "-journal"] {
        directory.validate_existing_file(&format!("{name}{suffix}"))?;
    }
    directory.open_file(name, true, false, true)?;
    let db = Connection::open(path).map_err(error)?;
    db.busy_timeout(Duration::from_secs(1)).map_err(error)?;
    db.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=NORMAL; CREATE TABLE IF NOT EXISTS documents(key TEXT PRIMARY KEY, fetched INTEGER NOT NULL, digest BLOB NOT NULL, body BLOB NOT NULL);").map_err(error)?;
    let page_size: i64 = db
        .pragma_query_value(None, "page_size", |r| r.get(0))
        .map_err(error)?;
    db.pragma_update(None, "max_page_count", 384 * 1024 * 1024 / page_size)
        .map_err(error)?;
    Ok(db)
}
pub(super) fn get(
    db: &mut Connection,
    key: &str,
    ttl: Duration,
    limit: usize,
) -> Result<Option<Vec<u8>>> {
    let now = chrono::Utc::now().timestamp();
    let oldest = now.saturating_sub(ttl.as_secs().min(i64::MAX as u64) as i64);
    let value:Option<(Vec<u8>,Vec<u8>)>=db.query_row("SELECT body,digest FROM documents WHERE key=? AND fetched>=? AND fetched<=? AND length(body)<=?",params![key,oldest,now,limit.min(64*1024*1024) as i64],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(error)?;
    Ok(value
        .and_then(|(bytes, digest)| (Sha256::digest(&bytes).as_slice() == digest).then_some(bytes)))
}
pub(super) fn put(db: &mut Connection, key: &str, body: &[u8]) -> Result<()> {
    if key.is_empty() || key.len() > 1024 || body.len() > 64 * 1024 * 1024 {
        return Err(error("Cache entry exceeds bounds."));
    }
    let tx = db.transaction().map_err(error)?;
    tx.execute("DELETE FROM documents WHERE key=?", [key])
        .map_err(error)?;
    let (mut count, mut bytes): (i64, i64) = tx
        .query_row(
            "SELECT count(*),coalesce(sum(length(body)),0) FROM documents",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(error)?;
    while count >= 512 || bytes + body.len() as i64 > PAYLOAD_LIMIT as i64 {
        let (old, len): (String, i64) = tx
            .query_row(
                "SELECT key,length(body) FROM documents ORDER BY fetched,key LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(error)?;
        tx.execute("DELETE FROM documents WHERE key=?", [old])
            .map_err(error)?;
        count -= 1;
        bytes -= len;
    }
    tx.execute(
        "INSERT INTO documents VALUES (?,?,?,?)",
        params![
            key,
            chrono::Utc::now().timestamp(),
            Sha256::digest(body).as_slice(),
            body
        ],
    )
    .map_err(error)?;
    tx.commit().map_err(error)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_bytes_require_bounds_freshness_and_matching_digest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap().join("cache/data.sqlite");
        let mut db = open(&path).unwrap();
        put(&mut db, "public", b"{\"id\":1}").unwrap();
        assert_eq!(
            get(&mut db, "public", Duration::from_secs(60), 100)
                .unwrap()
                .unwrap(),
            b"{\"id\":1}"
        );
        assert!(
            get(&mut db, "public", Duration::from_secs(60), 1)
                .unwrap()
                .is_none()
        );
        db.execute("UPDATE documents SET body=CAST('tampered' AS BLOB)", [])
            .unwrap();
        assert!(
            get(&mut db, "public", Duration::from_secs(60), 100)
                .unwrap()
                .is_none()
        );
        put(&mut db, "public", b"valid").unwrap();
        db.execute("UPDATE documents SET fetched=fetched-120", [])
            .unwrap();
        assert!(
            get(&mut db, "public", Duration::from_secs(60), 100)
                .unwrap()
                .is_none()
        );
        db.execute("UPDATE documents SET fetched=fetched+3600", [])
            .unwrap();
        assert!(
            get(&mut db, "public", Duration::from_secs(60), 100)
                .unwrap()
                .is_none()
        );
    }
    #[cfg(unix)]
    #[test]
    fn cache_refuses_linked_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        std::os::unix::fs::symlink("/tmp/unrelated-cache", path.join("public.sqlite-journal"))
            .unwrap();
        assert!(open(&path.join("public.sqlite")).is_err());
        assert!(!path.join("public.sqlite").exists());
    }
    #[test]
    fn entry_limit_evicts_old_records_transactionally() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap().join("cache/data.sqlite");
        let mut db = open(&path).unwrap();
        for i in 0..513 {
            put(&mut db, &format!("k{i:03}"), b"data").unwrap();
        }
        let count: i64 = db
            .query_row("SELECT count(*) FROM documents", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 512);
        assert!(
            get(&mut db, "k000", Duration::from_secs(60), 100)
                .unwrap()
                .is_none()
        );
        assert!(
            get(&mut db, "k512", Duration::from_secs(60), 100)
                .unwrap()
                .is_some()
        );
    }
}
