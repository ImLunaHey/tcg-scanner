use crate::{Database, decode, encode, events::append, key, storage};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use scanner_types::{
    error::{Result, ScannerError},
    service::{ProfilePatch, ScanProfile},
};
use serde_json::Value;
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

impl Database {
    pub fn profiles(&self) -> Result<Vec<Value>> {
        self.read(profiles)
    }
    pub fn profile(&self, id: Uuid) -> Result<Option<Value>> {
        self.read(|db| profile(db, id))
    }
    pub fn find_profile(&self, identifier: &str) -> Result<ScanProfile> {
        self.read(|db| typed(resolve(db, identifier)?))
    }
    pub fn save_profile(&self, id: Uuid, value: &Value) -> Result<()> {
        let mut value = value.clone();
        value["id"] = Value::String(key(id));
        self.write(|tx| save(tx, &value))
    }
    pub fn create_profile(&self, value: &ScanProfile) -> Result<ScanProfile> {
        value.validate()?;
        self.write(|tx| {
            if let Some(existing) = profile(tx, value.id)? {
                let existing = typed(existing)?;
                if existing == *value {
                    return Ok(existing);
                }
                return Err(ScannerError::new(
                    "conflict",
                    "Profile ID already exists with different settings.",
                ));
            }
            save(tx, &serde_json::to_value(value).map_err(storage)?)?;
            Ok(value.clone())
        })
    }
    pub fn update_profile(&self, identifier: &str, patch: &ProfilePatch) -> Result<ScanProfile> {
        self.write(|tx| {
            let mut original = resolve(tx, identifier)?;
            let mut updated = typed(original.clone())?;
            patch.apply(&mut updated)?;
            // Preserve additive fields while replacing only the known controls.
            let fields = serde_json::to_value(&updated).map_err(storage)?;
            for (name, value) in fields
                .as_object()
                .ok_or_else(|| storage("Invalid profile object."))?
            {
                original[name] = value.clone();
            }
            save(tx, &original)?;
            Ok(updated)
        })
    }
    pub fn delete_profile(&self, id: Uuid) -> Result<()> {
        self.write(|tx| delete(tx, id))
    }
    pub fn remove_profile(&self, identifier: &str) -> Result<()> {
        self.write(|tx| delete(tx, typed(resolve(tx, identifier)?)?.id))
    }
}
fn profiles(db: &Connection) -> Result<Vec<Value>> {
    let mut query = db
        .prepare("SELECT record FROM profiles ORDER BY id")
        .map_err(storage)?;
    query
        .query_map([], |row| row.get::<_, Vec<u8>>(0))
        .map_err(storage)?
        .map(|row| decode(row.map_err(storage)?))
        .collect()
}
fn profile(db: &Connection, id: Uuid) -> Result<Option<Value>> {
    db.query_row("SELECT record FROM profiles WHERE id=?", [key(id)], |row| {
        row.get::<_, Vec<u8>>(0)
    })
    .optional()
    .map_err(storage)?
    .map(decode)
    .transpose()
}
fn resolve(db: &Connection, identifier: &str) -> Result<Value> {
    if identifier.is_empty() || identifier.len() > 512 || identifier.contains('\0') {
        return Err(ScannerError::new(
            "invalidParams",
            "Invalid profile identifier.",
        ));
    }
    if let Ok(id) = Uuid::parse_str(identifier)
        && let Some(profile) = profile(db, id)?
    {
        return Ok(profile);
    }
    let wanted = fold(identifier);
    let mut matches = profiles(db)?.into_iter().filter(|value| {
        value["name"]
            .as_str()
            .is_some_and(|name| fold(name) == wanted)
    });
    let first = matches
        .next()
        .ok_or_else(|| ScannerError::new("profileNotFound", "Unknown profile."))?;
    if matches.next().is_some() {
        return Err(ScannerError::new(
            "ambiguousProfile",
            "Multiple profiles have this name; use a UUID.",
        ));
    }
    Ok(first)
}
fn typed(value: Value) -> Result<ScanProfile> {
    serde_json::from_value(value).map_err(storage)
}
fn fold(value: &str) -> String {
    value.nfd().case_fold().nfc().collect()
}
fn save(tx: &Transaction<'_>, value: &Value) -> Result<()> {
    let requested = typed(value.clone())?;
    requested.validate()?;
    let existing = profiles(tx)?;
    let wanted = fold(&requested.name);
    for value in &existing {
        let other = typed(value.clone())?;
        if other.id != requested.id && fold(&other.name) == wanted {
            return Err(ScannerError::new(
                "conflict",
                "A profile with this name already exists.",
            ));
        }
    }
    if existing.len() >= 256 && profile(tx, requested.id)?.is_none() {
        return Err(ScannerError::new(
            "resourceLimit",
            "At most 256 profiles may be saved.",
        ));
    }
    let mut value = value.clone();
    value["id"] = Value::String(key(requested.id));
    tx.execute(
        "INSERT OR REPLACE INTO profiles VALUES (?,?)",
        params![key(requested.id), encode(&value)?],
    )
    .map_err(storage)?;
    append(tx, "profile.updated", &value)
}
fn delete(tx: &Transaction<'_>, id: Uuid) -> Result<()> {
    if tx
        .execute("DELETE FROM profiles WHERE id=?", [key(id)])
        .map_err(storage)?
        != 1
    {
        return Err(ScannerError::new("profileNotFound", "Unknown profile ID."));
    }
    append(tx, "profile.deleted", &Value::String(key(id)))
}
#[cfg(test)]
mod tests {
    use super::*;
    use scanner_types::{scan::OriginalFormat, service::ScanSettings};
    fn named(name: &str) -> ScanProfile {
        ScanProfile {
            id: Uuid::new_v4(),
            name: name.into(),
            settings: ScanSettings {
                dpi: 600,
                duplex: false,
                format: OriginalFormat::Png,
            },
        }
    }
    #[test]
    fn creates_are_idempotent_and_unicode_names_are_unique() {
        let db = Database::memory().unwrap();
        let first = named("Straße");
        db.create_profile(&first).unwrap();
        let cursor = db.events(0, 100).unwrap().latest_cursor;
        assert_eq!(db.create_profile(&first).unwrap(), first);
        assert_eq!(db.events(0, 100).unwrap().latest_cursor, cursor);
        assert_eq!(db.find_profile("STRASSE").unwrap(), first);
        assert_eq!(
            db.create_profile(&named("strasse")).unwrap_err().kind,
            "conflict"
        );
        let composed = named("Café");
        db.create_profile(&composed).unwrap();
        assert_eq!(db.find_profile("CAFE\u{301}").unwrap(), composed);
        assert!(db.find_profile("cafe").is_err());
    }
    #[test]
    fn partial_updates_preserve_additive_metadata_and_rollback_invalid_changes() {
        let db = Database::memory().unwrap();
        let first = named("Cards");
        let mut value = serde_json::to_value(&first).unwrap();
        value["future"] = "preserve".into();
        db.save_profile(first.id, &value).unwrap();
        db.update_profile(
            "cards",
            &ProfilePatch {
                duplex: Some(true),
                ..ProfilePatch::default()
            },
        )
        .unwrap();
        let stored = db.profile(first.id).unwrap().unwrap();
        assert_eq!(stored["future"], "preserve");
        assert_eq!(stored["dpi"], 600);
        assert!(
            db.update_profile(
                "cards",
                &ProfilePatch {
                    dpi: Some(99),
                    ..ProfilePatch::default()
                }
            )
            .is_err()
        );
        assert_eq!(db.profile(first.id).unwrap().unwrap(), stored);
        assert!(
            db.update_profile("cards", &ProfilePatch::default())
                .is_err()
        );
    }
    #[test]
    fn concurrent_name_reservations_and_patches_are_atomic() {
        use std::sync::{Arc, Barrier};
        let db = Arc::new(Database::memory().unwrap());
        let barrier = Arc::new(Barrier::new(2));
        let tasks: Vec<_> = (0..2)
            .map(|_| {
                let db = db.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    db.create_profile(&named("Shared"))
                })
            })
            .collect();
        let outcomes: Vec<_> = tasks.into_iter().map(|task| task.join().unwrap()).collect();
        assert_eq!(outcomes.iter().filter(|value| value.is_ok()).count(), 1);
        let tasks: Vec<_> = [
            ProfilePatch {
                dpi: Some(300),
                ..ProfilePatch::default()
            },
            ProfilePatch {
                duplex: Some(true),
                ..ProfilePatch::default()
            },
        ]
        .into_iter()
        .map(|patch| {
            let db = db.clone();
            std::thread::spawn(move || db.update_profile("Shared", &patch))
        })
        .collect();
        for task in tasks {
            task.join().unwrap().unwrap();
        }
        let saved = db.find_profile("shared").unwrap();
        assert_eq!(saved.settings.dpi, 300);
        assert!(saved.settings.duplex);
    }
}
