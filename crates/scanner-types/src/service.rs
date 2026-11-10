use crate::{
    error::{Result, ScannerError},
    scan::{OriginalFormat, ScanRecord, ScanRequest, ScanState},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    pub schema_version: u32,
    pub device: crate::device::Device,
    pub service: ServiceStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inspection: Option<crate::device::Inspection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_settings: Option<ScanSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready: Option<bool>,
    pub warnings: Vec<String>,
}

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    #[serde(default)]
    pub button_observation: bool,
    pub stopping: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistence_error: Option<ScannerError>,
    pub storage_root: String,
    pub requires_restart: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilePatch {
    pub name: Option<String>,
    pub dpi: Option<u32>,
    pub duplex: Option<bool>,
    pub format: Option<OriginalFormat>,
}
impl ProfilePatch {
    pub fn apply(&self, profile: &mut ScanProfile) -> Result<()> {
        if self.name.is_none()
            && self.dpi.is_none()
            && self.duplex.is_none()
            && self.format.is_none()
        {
            return Err(ScannerError::new(
                "invalidSettings",
                "Provide at least one profile field to update.",
            ));
        }
        if let Some(name) = &self.name {
            profile.name = name.clone();
        }
        if let Some(dpi) = self.dpi {
            profile.settings.dpi = dpi;
        }
        if let Some(duplex) = self.duplex {
            profile.settings.duplex = duplex;
        }
        if let Some(format) = self.format {
            profile.settings.format = format;
        }
        profile.validate()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobPage {
    pub schema_version: u32,
    pub jobs: Vec<JobSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatus {
    pub schema_version: u32,
    pub storage_root: String,
    #[serde(rename = "requiredSMBShare", skip_serializing_if = "Option::is_none")]
    pub required_smb_share: Option<String>,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ScannerError>,
}
