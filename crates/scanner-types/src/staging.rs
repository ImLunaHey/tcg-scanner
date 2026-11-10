//! Store transfer progress; remote staging remains separate from inventory.
use crate::error::ScannerError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferState {
    #[default]
    NotSent,
    Sending,
    Sent,
    Failed,
}
impl TransferState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotSent => "notSent",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Failed => "failed",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferStatus {
    pub schema_version: u32,
    #[serde(rename = "jobID")]
    pub job_id: Uuid,
    pub configured: bool,
    pub state: TransferState,
    pub total_cards: usize,
    pub sent_cards: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preserved_cards: Option<Vec<String>>,
    #[serde(default, rename = "reviewURL", skip_serializing_if = "Option::is_none")]
    pub review_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ScannerError>,
    pub updated_at: DateTime<Utc>,
}
impl TransferStatus {
    pub fn new(job_id: Uuid, configured: bool) -> Self {
        Self {
            schema_version: 1,
            job_id,
            configured,
            state: TransferState::NotSent,
            total_cards: 0,
            sent_cards: 0,
            preserved_cards: None,
            review_url: None,
            error: None,
            updated_at: Utc::now(),
        }
    }
}
