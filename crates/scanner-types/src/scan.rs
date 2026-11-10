use crate::{
    Extensions,
    cards::CardLanguage,
    device::Device,
    error::{Result, ScannerError},
    images::{OriginalImage, ProcessedImage},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OriginalFormat {
    Tiff,
    #[default]
    Png,
}

impl OriginalFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Tiff => "tiff",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanBackground {
    Black,
    White,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanState {
    #[default]
    Queued,
    Preparing,
    Scanning,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

impl ScanState {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Preparing => "preparing",
            Self::Scanning => "scanning",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRequest {
    #[serde(rename = "requestID")]
    pub request_id: Uuid,
    #[serde(default, rename = "deviceID", skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_directory: Option<String>,
    #[serde(default, rename = "profileID", skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_device_settings: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepare_cards: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_language: Option<CardLanguage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<ScanBackground>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overscan: Option<bool>,
    pub dpi: u32,
    pub duplex: bool,
    pub format: OriginalFormat,
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl Default for ScanRequest {
    fn default() -> Self {
        Self {
            request_id: Uuid::new_v4(),
            device_id: None,
            output_directory: None,
            profile_id: None,
            use_device_settings: None,
            prepare_cards: None,
            card_language: None,
            background: None,
            overscan: None,
            dpi: 600,
            duplex: false,
            format: OriginalFormat::Png,
            extensions: Extensions::new(),
        }
    }
}

impl ScanRequest {
    pub fn validate(&self) -> Result<()> {
        let invalid = |message| ScannerError::new("invalidSettings", message);
        if !(150..=600).contains(&self.dpi) {
            return Err(invalid("DPI must be between 150 and 600."));
        }
        if self.overscan == Some(true) && self.background == Some(ScanBackground::White) {
            return Err(invalid("Overscan requires black backing."));
        }
        if self.profile_id.is_some() && self.use_device_settings == Some(true) {
            return Err(invalid("Choose a profile or device defaults."));
        }
        if self
            .device_id
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 512 || s.contains('\0'))
        {
            return Err(invalid("Invalid scanner ID."));
        }
        if self
            .output_directory
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 2048 || s.contains('\0'))
        {
            return Err(invalid("Invalid output directory."));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRecord {
    #[serde(default = "crate::schema_version")]
    pub schema_version: u32,
    pub id: Uuid,
    pub device: Device,
    pub request: ScanRequest,
    pub state: ScanState,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_number: Option<u64>,
    pub output_directory: String,
    #[serde(default)]
    pub images: Vec<OriginalImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processed_images: Option<Vec<ProcessedImage>>,
    #[serde(default)]
    pub errors: Vec<ScannerError>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub applied_settings: Extensions,
    #[serde(default)]
    pub session_closed: bool,
    /// Additive records (identification, finish, condition, continuation) remain
    /// lossless while their owning modules acquire typed interfaces.
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl ScanRecord {
    pub fn new(device: Device, request: ScanRequest, directory: String) -> Self {
        Self {
            schema_version: 1,
            id: request.request_id,
            device,
            request,
            state: ScanState::Queued,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            batch_number: None,
            output_directory: directory,
            images: vec![],
            processed_images: None,
            errors: vec![],
            warnings: vec![],
            applied_settings: Extensions::new(),
            session_closed: false,
            extensions: Extensions::new(),
        }
    }
    pub fn batch_label(&self) -> String {
        self.batch_number
            .map(|n| format!("B-{n:03}"))
            .unwrap_or_else(|| format!("batch-{}", self.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_conflicting_settings_before_capture() {
        let mut request = ScanRequest {
            overscan: Some(true),
            background: Some(ScanBackground::White),
            ..ScanRequest::default()
        };
        assert!(request.validate().is_err());
        request.background = Some(ScanBackground::Black);
        assert!(request.validate().is_ok());
        request.profile_id = Some(Uuid::new_v4());
        request.use_device_settings = Some(true);
        assert!(request.validate().is_err());
    }
    #[test]
    fn swift_acronyms_and_unknown_metadata_survive() {
        let mut record = ScanRecord::new(Device::default(), ScanRequest::default(), "B-001".into());
        let evidence = serde_json::json!([{"state":"confirmed","selectedCardID":"sv09-120","futureField":{"x":7}}]);
        record
            .extensions
            .insert("cardIdentifications".into(), evidence.clone());
        let value = serde_json::to_value(&record).unwrap();
        assert!(value["request"]["requestID"].is_string());
        assert!(value["request"].get("requestId").is_none());
        let restored: ScanRecord = serde_json::from_value(value).unwrap();
        assert_eq!(restored.extensions["cardIdentifications"], evidence);
    }
}

#[cfg(test)]
mod saved_manifest {
    use super::*;
    #[test]
    fn existing_manifest_preserves_images_reviews_and_additive_metadata() {
        let Some(path) = std::env::var_os("SCANNER_COMPAT_MANIFEST") else {
            return;
        };
        let bytes = std::fs::read(path).unwrap();
        let before: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let record: ScanRecord = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(record.schema_version, 1);
        let after = serde_json::to_value(&record).unwrap();
        for key in [
            "cardIdentifications",
            "cardGames",
            "continuations",
            "appliedSettings",
            "images",
        ] {
            assert_eq!(before.get(key), after.get(key), "Lost {key}");
        }
        for (old, new) in before["processedImages"]
            .as_array()
            .into_iter()
            .flatten()
            .zip(after["processedImages"].as_array().into_iter().flatten())
        {
            assert_eq!(old["sourceSHA256"], new["sourceSHA256"]);
            assert_eq!(old["image"], new["image"]);
            let old_plan: crate::images::CardPreparation =
                serde_json::from_value(old["cardPreparation"].clone()).unwrap();
            let new_plan: crate::images::CardPreparation =
                serde_json::from_value(new["cardPreparation"].clone()).unwrap();
            assert_eq!(old_plan, new_plan);
        }
    }
}
