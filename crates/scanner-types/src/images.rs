use crate::{
    Extensions,
    error::{Result, ScannerError},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OriginalImage {
    pub sequence: u32,
    pub relative_path: String,
    pub bytes: u64,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub image_count: u32,
    pub uniform_type: String,
    pub side: String,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageCrop {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Default for ImageCrop {
    fn default() -> Self {
        Self {
            x: 0.,
            y: 0.,
            width: 1.,
            height: 1.,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageEditRequest {
    pub id: Uuid,
    #[serde(rename = "jobID")]
    pub job_id: Uuid,
    pub sequence: u32,
    pub crop: ImageCrop,
    pub quarter_turns: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deskew_degrees: Option<f64>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl ImageEditRequest {
    pub fn validate(&self) -> Result<()> {
        let c = self.crop;
        let angle = self.deskew_degrees.unwrap_or(0.);
        if !(1..=1000).contains(&self.sequence)
            || self.quarter_turns > 3
            || !angle.is_finite()
            || angle.abs() > 12.
            || ![c.x, c.y, c.width, c.height].iter().all(|x| x.is_finite())
            || c.x < 0.
            || c.y < 0.
            || c.width <= 0.
            || c.height <= 0.
            || c.x + c.width > 1.000000001
            || c.y + c.height > 1.000000001
        {
            return Err(ScannerError::new(
                "invalidParams",
                "Crop, rotation or deskew is outside supported bounds.",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardPreparation {
    pub crop: ImageCrop,
    pub quarter_turns: u8,
    pub orientation_verified: bool,
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_crop: Option<ImageCrop>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deskew_degrees: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation_basis: Option<String>,
    pub method: String,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessedImage {
    pub request: ImageEditRequest,
    pub created_at: DateTime<Utc>,
    #[serde(rename = "sourceSHA256")]
    pub source_sha256: String,
    pub image: OriginalImage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_preparation: Option<CardPreparation>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crop_coordinates_survive_direct_and_value_json_paths_exactly() {
        let json = r#"{"x":0.36903018872890414,"y":0.0621360712626452,"width":0.24966967649723099,"height":0.24777452823137183}"#;
        let direct: ImageCrop = serde_json::from_str(json).unwrap();
        let value: ImageCrop = serde_json::from_value(serde_json::from_str(json).unwrap()).unwrap();
        assert_eq!(direct, value);
        let restored: ImageCrop =
            serde_json::from_str(&serde_json::to_string(&direct).unwrap()).unwrap();
        assert_eq!(direct, restored);
    }
    #[test]
    fn rejects_nonfinite_and_outside_crops() {
        let mut request = ImageEditRequest {
            id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            sequence: 1,
            crop: ImageCrop::default(),
            quarter_turns: 0,
            deskew_degrees: None,
            extensions: Extensions::new(),
        };
        assert!(request.validate().is_ok());
        request.crop.width = f64::NAN;
        assert!(request.validate().is_err());
        request.crop.width = 1.;
        request.crop.x = 0.1;
        assert!(request.validate().is_err());
        request.crop.x = 0.;
        request.quarter_turns = 4;
        assert!(request.validate().is_err());
    }
}
