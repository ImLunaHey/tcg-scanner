use scanner_types::images::OriginalImage;
use scanner_types::{
    error::{Result, ScannerError},
    scan::ScanRecord,
};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

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
    let record = Arc::new(record.clone());
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
                    record: record.clone(),
                    image: image.clone(),
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
    record: Arc<ScanRecord>,
    image: OriginalImage,
}

impl ImageSource {
    pub fn thumbnail(self) -> Result<Vec<u8>> {
        let parent = Path::new(&self.record.output_directory)
            .parent()
            .ok_or_else(|| ScannerError::new("storage", "Saved batch path is invalid."))?;
        let root = scanner_files::StorageRoot::open(parent)?;
        let bytes = root.read_image(&self.record, &self.image, 32 * 1024 * 1024)?;
        Ok(scanner_images::thumbnail::thumbnail(&bytes, 1024, false)?.bytes)
    }
}
