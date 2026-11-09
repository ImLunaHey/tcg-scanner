use crate::{
    error::{Result, ScannerError},
    scan::{OriginalFormat, ScanRecord, ScanRequest, ScanState},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScanSettings {
    pub dpi: u32,
    pub duplex: bool,
    pub format: OriginalFormat,
}
impl ScanSettings {
    pub fn validate(&self) -> Result<()> {
        ScanRequest {
            dpi: self.dpi,
            duplex: self.duplex,
            format: self.format,
            ..ScanRequest::default()
        }
        .validate()
    }
    pub fn apply(&self, request: &mut ScanRequest) {
        request.dpi = self.dpi;
        request.duplex = self.duplex;
        request.format = self.format;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanProfile {
    pub id: Uuid,
    pub name: String,
    #[serde(flatten)]
    pub settings: ScanSettings,
}
impl ScanProfile {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() || self.name.len() > 128 {
            return Err(ScannerError::new(
                "invalidSettings",
                "Profile name must contain 1–128 bytes.",
            ));
        }
        self.settings.validate()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub images_saved: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_images: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSummary {
    pub id: Uuid,
    #[serde(rename = "scannerID")]
    pub scanner_id: String,
    #[serde(rename = "profileID", skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<Uuid>,
    #[serde(rename = "batchID")]
    pub batch_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_number: Option<u64>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    pub state: ScanState,
    pub progress: ScanProgress,
    pub output_directory: String,
    pub errors: Vec<ScannerError>,
}
impl From<&ScanRecord> for JobSummary {
    fn from(record: &ScanRecord) -> Self {
        Self {
            id: record.id,
            scanner_id: record.device.id.clone(),
            profile_id: record
                .extensions
                .get("profile")
                .and_then(|p| p["id"].as_str())
                .and_then(|id| Uuid::parse_str(id).ok()),
            batch_id: record.id,
            batch_number: record.batch_number,
            created_at: record.created_at,
            started_at: record.started_at,
            completed_at: record.completed_at,
            state: record.state,
            progress: ScanProgress {
                images_saved: record.images.len(),
                total_images: None,
            },
            output_directory: record.output_directory.clone(),
            errors: record.errors.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub schema_version: u32,
    #[serde(rename = "activeJobID", skip_serializing_if = "Option::is_none")]
    pub active_job_id: Option<Uuid>,
    pub queued_jobs: usize,
    pub paused: bool,
    pub stopping: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistence_error: Option<ScannerError>,
    pub storage_root: String,
    pub requires_restart: bool,
}
