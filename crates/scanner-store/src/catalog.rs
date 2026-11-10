use crate::{Database, decode, encode, events::append, storage};
use rusqlite::{OptionalExtension, params};
use scanner_types::{
    device::Device,
    error::{Result, ScannerError},
};
use serde_json::Value;

impl Database {
    pub fn device_settings(&self, id: &str) -> Result<Option<Value>> {
        self.read(|db| {
            db.query_row(
                "SELECT record FROM deviceSettings WHERE deviceID=?",
                [id],
                |r| r.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(storage)?
            .map(decode)
            .transpose()
        })
    }
    pub fn save_device_settings(&self, id: &str, value: &Value) -> Result<()> {
        validate_settings(value)?;
        self.write(|tx| {
            tx.execute(
                "INSERT OR REPLACE INTO deviceSettings VALUES (?,?)",
                params![id, encode(value)?],
            )
            .map_err(storage)?;
            append(
                tx,
                "settings.updated",
                &serde_json::json!({"deviceID":id,"configuredSettings":value}),
            )
        })
    }
    pub fn save_inspection(&self, id: &str, value: &Value) -> Result<()> {
        self.write(|tx| {
            tx.execute(
                "INSERT OR REPLACE INTO capabilities VALUES (?,?)",
                params![id, encode(value)?],
            )
            .map_err(storage)?;
            Ok(())
        })
    }
    pub fn inspection(&self, id: &str) -> Result<Option<Value>> {
        self.read(|db| {
            db.query_row(
                "SELECT record FROM capabilities WHERE deviceID=?",
                [id],
                |r| r.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(storage)?
            .map(decode)
            .transpose()
        })
    }
    pub fn observe_devices(&self, devices: &[Device]) -> Result<()> {
        self.write(|tx| {
            let mut query = tx
                .prepare("SELECT id FROM devices WHERE connected=1")
                .map_err(storage)?;
            let old = query
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(storage)?
                .collect::<std::result::Result<std::collections::HashSet<_>, _>>()
                .map_err(storage)?;
            tx.execute("UPDATE devices SET connected=0", [])
                .map_err(storage)?;
            for device in devices {
                tx.execute(
                    "INSERT OR REPLACE INTO devices VALUES (?,?,1)",
                    params![device.id, encode(device)?],
                )
                .map_err(storage)?;
                if !old.contains(&device.id) {
                    append(
                        tx,
                        "device.connected",
                        &serde_json::to_value(device).map_err(storage)?,
                    )?;
                }
            }
            for id in old {
                if !devices.iter().any(|d| d.id == id) {
                    append(tx, "device.disconnected", &Value::String(id))?;
                }
            }
            Ok(())
        })
    }
}
fn validate_settings(value: &Value) -> Result<()> {
    if value["dpi"]
        .as_u64()
        .is_none_or(|n| !(150..=600).contains(&n))
        || !value["duplex"].is_boolean()
        || !matches!(value["format"].as_str(), Some("png" | "tiff"))
    {
        return Err(ScannerError::new(
            "invalidSettings",
            "Expected supported DPI, duplex and PNG/TIFF format.",
        ));
    }
    Ok(())
}
