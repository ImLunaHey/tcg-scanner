use scanner_types::{
    error::{Result, ScannerError},
    scan::ScanRecord,
};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

pub struct Card {
    pub sequence: u32,
    pub name: String,
    pub subtitle: String,
    pub status: String,
    pub evidence: Vec<String>,
    pub source: ImageSource,
}

pub fn read_library() -> Result<Vec<ScanRecord>> {
    if let Some(path) = std::env::var_os("SCANNER_UI_MANIFEST") {
        let bytes = std::fs::read(path).map_err(|e| ScannerError::new("storage", e.to_string()))?;
        let record = serde_json::from_slice(&bytes)
            .map_err(|e| ScannerError::new("storage", e.to_string()))?;
        return Ok(vec![record]);
    }
    let path = std::env::var_os("SCANNER_DATABASE")
        .map(PathBuf::from)
        .or_else(|| dirs::data_local_dir().map(|p| p.join("Scanner/state/scanner.sqlite")))
        .ok_or_else(|| {
            ScannerError::new("storage", "Application data directory is unavailable.")
        })?;
    // Read-only until the service owns all mutations; never initializes or migrates a live ledger.
    scanner_store::Database::read_only(&path)?.jobs(100, 0)
}

pub fn cards(record: &ScanRecord) -> Vec<Card> {
    record
        .images
        .iter()
        .map(|original| {
            let review = record
                .extensions
                .get("cardIdentifications")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items
                        .iter()
                        .rev()
                        .find(|v| v["request"]["sequence"] == original.sequence)
                });
            let candidate = review.and_then(|r| {
                let candidates = r["evidence"]["candidates"].as_array()?;
                match r["selectedCardID"].as_str() {
                    Some(id) => candidates.iter().find(|card| card["id"] == id),
                    None => candidates.first(),
                }
            });
            let image = record
                .processed_images
                .as_ref()
                .and_then(|items| {
                    items
                        .iter()
                        .rev()
                        .find(|edit| edit.request.sequence == original.sequence)
                })
                .map(|edit| &edit.image)
                .unwrap_or(original);
            Card {
                sequence: original.sequence,
                name: candidate
                    .and_then(|c| c["name"].as_str())
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("Image {:03}", original.sequence)),
                subtitle: candidate
                    .map(|c| format!("{} · {}", field(c, "setName"), field(c, "localID")))
                    .unwrap_or_else(|| {
                        format!("{} × {} · {}", image.width, image.height, original.side)
                    }),
                status: review
                    .map(|r| field(r, "state"))
                    .unwrap_or("Not identified")
                    .to_owned(),
                evidence: candidate
                    .and_then(|c| c["evidence"].as_array())
                    .into_iter()
                    .flatten()
                    .chain(
                        review
                            .and_then(|r| r["evidence"]["warnings"].as_array())
                            .into_iter()
                            .flatten(),
                    )
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
                source: ImageSource {
                    directory: PathBuf::from(&record.output_directory),
                    relative: image.relative_path.clone(),
                    bytes: image.bytes,
                    sha256: image.sha256.clone(),
                },
            }
        })
        .collect()
}

fn field<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}

#[derive(Clone)]
pub struct ImageSource {
    directory: PathBuf,
    relative: String,
    bytes: u64,
    sha256: String,
}

impl ImageSource {
    pub fn thumbnail(self) -> Result<Vec<u8>> {
        use sha2::{Digest, Sha256};
        use std::io::Read;
        let invalid = || {
            ScannerError::new(
                "storage",
                "Saved image is unavailable or failed its integrity check.",
            )
        };
        if self.bytes > 32 * 1024 * 1024 {
            return Err(invalid());
        }
        let path = image_path(&self.directory, &self.relative).ok_or_else(invalid)?;
        let file = std::fs::File::open(path).map_err(|_| invalid())?;
        let info = file.metadata().map_err(|_| invalid())?;
        if !info.is_file() || info.len() != self.bytes {
            return Err(invalid());
        }
        let mut bytes = Vec::new();
        file.take(self.bytes + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| invalid())?;
        if bytes.len() as u64 != self.bytes
            || format!("{:x}", Sha256::digest(&bytes)) != self.sha256
        {
            return Err(invalid());
        }
        Ok(scanner_images::thumbnail::thumbnail(&bytes, 1024, false)?.bytes)
    }
}

fn image_path(directory: &Path, relative: &str) -> Option<PathBuf> {
    let relative = Path::new(relative);
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
        || relative.as_os_str().is_empty()
    {
        return None;
    }
    let root = directory.canonicalize().ok()?;
    let path = root.join(relative).canonicalize().ok()?;
    path.starts_with(&root).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absolute_and_parent_paths_are_not_accepted() {
        assert!(image_path(Path::new("/tmp"), "../etc/passwd").is_none());
        assert!(image_path(Path::new("/tmp"), "/etc/passwd").is_none());
    }
}
