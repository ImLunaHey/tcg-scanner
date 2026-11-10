use crate::{
    Extensions,
    cards::CardLanguage,
    error::{Result, ScannerError},
    recognition::{CardCondition, CardFinish, RecognitionEvidence},
    scan::ScanRecord,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IdentificationState {
    #[default]
    Processing,
    ReviewRequired,
    AwaitingCatalogue,
    NoMatch,
    Failed,
    Confirmed,
    Rejected,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentificationRequest {
    pub id: Uuid,
    #[serde(rename = "jobID")]
    pub job_id: Uuid,
    pub sequence: u32,
    pub language: CardLanguage,
    #[serde(flatten)]
    pub extensions: Extensions,
}
impl IdentificationRequest {
    pub fn validate(&self) -> Result<()> {
        if !(1..=1000).contains(&self.sequence) {
            return Err(ScannerError::new(
                "invalidParams",
                "Sequence must be 1–1000.",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardIdentification {
    #[serde(default = "crate::schema_version")]
    pub schema_version: u32,
    pub request: IdentificationRequest,
    #[serde(rename = "sourceSHA256")]
    pub source_sha256: String,
    #[serde(
        default,
        rename = "sourceEditID",
        skip_serializing_if = "Option::is_none"
    )]
    pub source_edit_id: Option<Uuid>,
    pub state: IdentificationState,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalogue_retry_after: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<RecognitionEvidence>,
    #[serde(
        default,
        rename = "selectedCardID",
        skip_serializing_if = "Option::is_none"
    )]
    pub selected_card_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ScannerError>,
    pub method: String,
    #[serde(default)]
    pub finish: CardFinish,
    #[serde(default)]
    pub condition: CardCondition,
    pub assessment_source: String,
    #[serde(flatten)]
    pub extensions: Extensions,
}
impl CardIdentification {
    pub fn pending(
        request: IdentificationRequest,
        source_sha256: String,
        source_edit_id: Option<Uuid>,
    ) -> Self {
        Self {
            schema_version: 1,
            request,
            source_sha256,
            source_edit_id,
            state: IdentificationState::Processing,
            created_at: Utc::now(),
            completed_at: None,
            reviewed_at: None,
            catalogue_retry_after: None,
            evidence: None,
            selected_card_id: None,
            error: None,
            method: "tesseract-tcgdex-rust-v1".into(),
            finish: CardFinish::Unverified,
            condition: CardCondition::Unassessed,
            assessment_source: "manual-review".into(),
            extensions: Extensions::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewRequest {
    #[serde(rename = "jobID")]
    pub job_id: Uuid,
    pub sequence: u32,
    #[serde(rename = "identificationID")]
    pub identification_id: Uuid,
    #[serde(default, rename = "cardID")]
    pub card_id: Option<String>,
    #[serde(default)]
    pub finish: CardFinish,
    #[serde(default)]
    pub condition: CardCondition,
}
impl ReviewRequest {
    pub fn validate(&self) -> Result<()> {
        if !(1..=1000).contains(&self.sequence)
            || self
                .card_id
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.len() > 100 || v.contains('\0'))
        {
            return Err(ScannerError::new(
                "invalidParams",
                "Invalid sequence or candidate ID.",
            ));
        }
        Ok(())
    }
}
impl ScanRecord {
    pub fn identifications(&self) -> Result<Vec<CardIdentification>> {
        self.extensions
            .get("cardIdentifications")
            .map(|v| serde_json::from_value(v.clone()).map_err(storage))
            .unwrap_or(Ok(vec![]))
    }
    pub fn replace_identification(&mut self, result: &CardIdentification) -> Result<()> {
        let mut items = self.identifications()?;
        if let Some(previous) = items
            .iter_mut()
            .find(|i| i.request.sequence == result.request.sequence)
        {
            *previous = result.clone();
        } else {
            items.push(result.clone());
        }
        self.extensions.insert(
            "cardIdentifications".into(),
            serde_json::to_value(items).map_err(storage)?,
        );
        Ok(())
    }
}
fn storage(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("storage", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_identification_preserves_evidence_reviews_and_additive_assessments() {
        let Some(path) = std::env::var_os("SCANNER_COMPAT_MANIFEST") else {
            return;
        };
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let items = value["cardIdentifications"].as_array_mut().unwrap();
        assert!(!items.is_empty());
        for original in items {
            original["futureAssessment"] = serde_json::json!({"a":[1,2,3]});
            let result: CardIdentification = serde_json::from_value(original.clone()).unwrap();
            let restored = serde_json::to_value(result).unwrap();
            assert_eq!(
                serde_json::from_value::<IdentificationRequest>(original["request"].clone())
                    .unwrap(),
                serde_json::from_value::<IdentificationRequest>(restored["request"].clone())
                    .unwrap()
            );
            for key in [
                "sourceSHA256",
                "state",
                "selectedCardID",
                "finish",
                "condition",
                "finishEstimate",
                "conditionCheck",
                "futureAssessment",
            ] {
                assert_eq!(original.get(key), restored.get(key), "Lost {key}");
            }
            assert_eq!(
                original
                    .get("sourceEditID")
                    .and_then(|v| v.as_str())
                    .map(|s| Uuid::parse_str(s).unwrap()),
                restored
                    .get("sourceEditID")
                    .and_then(|v| v.as_str())
                    .map(|s| Uuid::parse_str(s).unwrap())
            );
            // OCR confidence narrows to the same Float used by Swift; all other
            // candidate and visual evidence is preserved without reinterpretation.
            assert_eq!(
                original["evidence"]["candidates"],
                restored["evidence"]["candidates"]
            );
        }
    }
}
