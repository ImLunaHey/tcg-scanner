use crate::{Database, decode, encode, storage};
use chrono::{DateTime, Utc};
use rusqlite::{Transaction, params};
use scanner_types::error::{Result, ScannerError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceEvent {
    pub cursor: i64,
    pub timestamp: DateTime<Utc>,
    pub kind: String,
    pub payload: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventPage {
    pub schema_version: u32,
    pub events: Vec<ServiceEvent>,
    pub next_cursor: i64,
    pub oldest_cursor: i64,
    pub latest_cursor: i64,
    pub reset_required: bool,
}

pub(crate) fn append(tx: &Transaction<'_>, kind: &str, payload: &Value) -> Result<()> {
    tx.execute(
        "INSERT INTO events(timestamp,kind,payload) VALUES (?,?,?)",
        params![
            Utc::now().timestamp_millis() as f64 / 1000.,
            kind,
            encode(payload)?
        ],
    )
    .map_err(storage)?;
    tx.execute(
        "DELETE FROM events WHERE cursor<=?",
        [tx.last_insert_rowid() - 10_000],
    )
    .map_err(storage)?;
    Ok(())
}
impl Database {
    pub fn event(&self, kind: &str, payload: &Value) -> Result<()> {
        self.write(|tx| append(tx, kind, payload))
    }
    pub fn events(&self, after: i64, limit: usize) -> Result<EventPage> {
        if after < 0 || !(1..=100).contains(&limit) {
            return Err(ScannerError::new(
                "invalidParams",
                "Invalid event cursor or limit.",
            ));
        }
        self.read(|db|{
            let mut q=db.prepare("SELECT cursor,timestamp,kind,payload FROM events WHERE cursor>? ORDER BY cursor LIMIT ?").map_err(storage)?;
            let rows=q.query_map(params![after,limit as i64],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,f64>(1)?,r.get::<_,String>(2)?,r.get::<_,Vec<u8>>(3)?))).map_err(storage)?;
            let mut events=vec![];
            for row in rows {
                let (cursor,seconds,kind,payload)=row.map_err(storage)?;
                let timestamp=DateTime::from_timestamp_millis((seconds*1000.) as i64).ok_or_else(||storage("Invalid event timestamp"))?;
                events.push(ServiceEvent{cursor,timestamp,kind,payload:decode(payload)?});
            }
            let (oldest,latest):(Option<i64>,Option<i64>)=db.query_row("SELECT MIN(cursor),MAX(cursor) FROM events",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(storage)?;
            let (oldest,latest)=(oldest.unwrap_or(0),latest.unwrap_or(0));
            Ok(EventPage{schema_version:1,next_cursor:events.last().map_or(after,|e|e.cursor),events,oldest_cursor:oldest,latest_cursor:latest,reset_required:after>latest||(after>0&&after<oldest-1)})
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn future_and_expired_cursors_require_snapshot_reset() {
        let db = Database::memory().unwrap();
        db.event("test", &Value::Null).unwrap();
        assert!(!db.events(0, 100).unwrap().reset_required);
        assert!(db.events(2, 100).unwrap().reset_required);
        db.write(|tx| {
            tx.execute("UPDATE events SET cursor=20", [])
                .map_err(storage)?;
            Ok(())
        })
        .unwrap();
        assert!(db.events(1, 100).unwrap().reset_required);
        assert!(!db.events(19, 100).unwrap().reset_required);
    }
}
