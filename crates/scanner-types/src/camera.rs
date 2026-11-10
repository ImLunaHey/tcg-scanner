//! Ephemeral lookup contracts. No batch, job or persistence identifiers.
use crate::{
    cards::{CardGame, CardLanguage},
    finish::FinishEstimate,
    recognition::{CardCandidate, RecognitionEvidence},
};
use serde::{Deserialize, Serialize};
pub const MAX_IMAGE_BYTES: usize = 512 * 1024;
pub const MAX_REQUEST_BYTES: usize = 768 * 1024;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CameraRequest {
    pub base64: String,
    #[serde(default)]
    pub language: CardLanguage,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CameraState {
    NoCard,
    CardBack,
    CodeCard,
    Matched,
    Ambiguous,
    NoMatch,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraIdentification {
    pub schema_version: u32,
    pub state: CameraState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game: Option<CardGame>,
    pub candidates: Vec<CardCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncatalogued_card: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy_symbol: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_estimate: Option<FinishEstimate>,
}
impl CameraIdentification {
    pub fn empty(state: CameraState) -> Self {
        Self {
            schema_version: 1,
            state,
            game: None,
            candidates: vec![],
            uncatalogued_card: None,
            energy_symbol: None,
            finish_estimate: None,
        }
    }
    pub fn from_evidence(evidence: RecognitionEvidence, game: CardGame) -> Self {
        let state = match evidence.candidates.len() {
            0 => CameraState::NoMatch,
            1 => CameraState::Matched,
            _ => CameraState::Ambiguous,
        };
        Self {
            game: Some(game),
            candidates: evidence.candidates,
            uncatalogued_card: evidence.extensions.get("uncataloguedCard").cloned(),
            energy_symbol: evidence.extensions.get("energySymbol").cloned(),
            ..Self::empty(state)
        }
    }
}
