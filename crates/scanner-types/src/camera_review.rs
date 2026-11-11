//! Reviewed camera identity shared by pricing and explicit inventory imports.
use crate::{
    cards::{CardGame, CardLanguage},
    error::{Result, ScannerError},
    finish::FinishEstimate,
    identification::{CardIdentification, IdentificationRequest},
    recognition::{CardCandidate, CardFinish},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraSelection {
    pub card: CardCandidate,
    pub game: CardGame,
    pub language: CardLanguage,
    #[serde(default)]
    pub finish: CardFinish,
    pub finish_estimate: Option<FinishEstimate>,
}
impl CameraSelection {
    pub fn validate(&self) -> Result<()> {
        let card = &self.card;
        let strings = [
            &card.id,
            &card.name,
            &card.set_id,
            &card.set_name,
            &card.local_id,
        ];
        if self.game == CardGame::Unknown
            || strings
                .iter()
                .any(|s| s.is_empty() || s.len() > 300 || s.contains('\0'))
            || !card
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            || serde_json::to_vec(self).map_or(true, |v| v.len() > 16 * 1024)
        {
            return Err(ScannerError::new(
                "invalidParams",
                "Select one supported catalogue card.",
            ));
        }
        Ok(())
    }
    pub fn identification(&self, job_id: Uuid, sequence: u32, sha: String) -> CardIdentification {
        let mut identity = CardIdentification::pending(
            IdentificationRequest {
                id: Uuid::new_v4(),
                job_id,
                sequence,
                language: self.language,
                extensions: Default::default(),
            },
            sha,
            None,
        );
        identity.finish = self.finish;
        if let Some(estimate) = &self.finish_estimate {
            identity
                .extensions
                .insert("finishEstimate".into(), serde_json::json!(estimate));
        }
        identity
    }
}
