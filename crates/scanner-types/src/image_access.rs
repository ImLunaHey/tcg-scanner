//! Image access envelopes shared by service transports and clients.
use crate::images::OriginalImage;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMetadata {
    pub schema_version: u32,
    #[serde(rename = "jobID")]
    pub job_id: Uuid,
    pub image: OriginalImage,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageChunk {
    pub schema_version: u32,
    pub metadata: ImageMetadata,
    pub offset: u64,
    pub next_offset: u64,
    pub eof: bool,
    pub base64: String,
    #[serde(rename = "chunkSHA256")]
    pub chunk_sha256: String,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThumbnailFormat {
    #[default]
    Png,
    Jpeg,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageThumbnail {
    pub schema_version: u32,
    pub base64: String,
    pub sha256: String,
    pub media_type: String,
    pub width: u32,
    pub height: u32,
}
