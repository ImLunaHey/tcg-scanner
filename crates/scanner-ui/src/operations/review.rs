use super::Command;
use crate::state::State;
use scanner_types::{
    identification::{IdentificationState, ReviewRequest},
    recognition::{CardCondition, CardFinish},
};
use serde_json::json;

impl State {
    pub(crate) fn update_review(&mut self) {
        let selected = self.cards.get(self.selected).map(|c| c.sequence);
        let review = self
            .record
            .as_ref()
            .and_then(|r| r.identifications().ok())
            .and_then(|items| {
                items
                    .into_iter()
                    .rev()
                    .find(|i| Some(i.request.sequence) == selected)
            });
        // Preserve unsaved choices while polling the same identification.
        let changed = self
            .ops
            .review
            .as_ref()
            .map(|r| (r.request.id, r.reviewed_at, r.completed_at))
            != review
                .as_ref()
                .map(|r| (r.request.id, r.reviewed_at, r.completed_at));
        if changed {
            self.ops.candidate = review.as_ref().and_then(|r| {
                r.selected_card_id.clone().or_else(|| {
                    r.evidence
                        .as_ref()?
                        .candidates
                        .first()
                        .map(|c| c.id.clone())
                })
            });
            self.ops.finish = review.as_ref().map(|r| r.finish).unwrap_or_default();
            self.ops.condition = review.as_ref().map(|r| r.condition).unwrap_or_default();
        }
        self.ops.review = review;
    }
    pub fn cycle_candidate(&mut self) {
        let Some(candidates) = self
            .ops
            .review
            .as_ref()
            .and_then(|r| r.evidence.as_ref())
            .map(|e| e.review_candidates())
        else {
            return;
        };
        if candidates.is_empty() {
            return;
        }
        let index = candidates
            .iter()
            .position(|c| Some(&c.id) == self.ops.candidate.as_ref())
            .map(|i| (i + 1) % candidates.len())
            .unwrap_or(0);
        self.ops.candidate = Some(candidates[index].id.clone());
    }
    pub fn cycle_finish(&mut self) {
        use CardFinish::*;
        self.ops.finish = match self.ops.finish {
            Unverified => Normal,
            Normal => Holo,
            Holo => ReverseHolo,
            ReverseHolo => Other,
            Other => Unverified,
        };
    }
    pub fn cycle_condition(&mut self) {
        use CardCondition::*;
        self.ops.condition = match self.ops.condition {
            Unassessed => NearMint,
            NearMint => LightlyPlayed,
            LightlyPlayed => ModeratelyPlayed,
            ModeratelyPlayed => HeavilyPlayed,
            HeavilyPlayed => Damaged,
            Damaged => Unassessed,
        };
    }
    pub fn confirm_review(&mut self) {
        if self.ops.candidate.is_some() {
            self.send_review(self.ops.candidate.clone());
        }
    }
    pub fn reject_review(&mut self) {
        self.send_review(None);
    }
    fn send_review(&mut self, card_id: Option<String>) {
        let Some(review) = &self.ops.review else {
            return;
        };
        if review.state == IdentificationState::Processing {
            return;
        }
        let request = ReviewRequest {
            job_id: review.request.job_id,
            sequence: review.request.sequence,
            identification_id: review.request.id,
            card_id,
            finish: self.ops.finish,
            condition: self.ops.condition,
        };
        self.submit(Command {
            method: "images.review",
            params: json!(request),
            job: Some(review.request.job_id),
            focus: false,
        });
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use scanner_types::{
        device::Device,
        identification::{CardIdentification, IdentificationRequest},
        scan::{ScanRecord, ScanRequest, ScanState},
    };
    use uuid::Uuid;
    fn record() -> ScanRecord {
        let mut record =
            ScanRecord::new(Device::default(), ScanRequest::default(), "/fixture".into());
        record.state = ScanState::Completed;
        record.images = vec![serde_json::from_value(json!({"sequence":1,"relativePath":"original.png","bytes":100,"sha256":"abc","width":30,"height":40,"imageCount":1,"uniformType":"public.png","side":"front"})).unwrap()];
        let mut review = CardIdentification::pending(
            IdentificationRequest {
                id: Uuid::new_v4(),
                job_id: record.id,
                sequence: 1,
                language: Default::default(),
                extensions: Default::default(),
            },
            "abc".into(),
            None,
        );
        review.state = IdentificationState::ReviewRequired;
        review.evidence = Some(serde_json::from_value(json!({"lines":[],"warnings":[],"orientation":0,"candidates":[
            {"id":"a","name":"First","localID":"1","setID":"s","setName":"Set","evidence":[]},
            {"id":"b","name":"Second","localID":"2","setID":"s","setName":"Set","evidence":[]}
        ]})).unwrap());
        record.replace_identification(&review).unwrap();
        record
    }
    #[test]
    fn visual_suggestions_require_selection_and_are_not_default_printings() {
        let mut record = record();
        let mut identity = record.identifications().unwrap().remove(0);
        let evidence = identity.evidence.as_mut().unwrap();
        let visual = evidence.candidates.remove(0);
        evidence.candidates.clear();
        evidence.extensions.insert(
            "visualMatches".into(),
            json!([{"candidate":visual,"similarity":0.9}]),
        );
        identity.state = IdentificationState::NoMatch;
        record.replace_identification(&identity).unwrap();
        let mut state = State::default();
        state.receive_record(record);
        assert!(state.ops.candidate.is_none());
        state.cycle_candidate();
        assert_eq!(state.ops.candidate.as_deref(), Some("a"));
        assert!(
            state
                .ops
                .review
                .as_ref()
                .unwrap()
                .evidence
                .as_ref()
                .unwrap()
                .candidates
                .is_empty()
        );
        assert_eq!(state.ops.finish, CardFinish::Unverified);
        assert_eq!(state.ops.condition, CardCondition::Unassessed);
    }
    #[test]
    fn background_refresh_keeps_manual_choices_until_a_new_review_is_saved() {
        let mut state = State::default();
        let mut record = record();
        state.receive_record(record.clone());
        state.cycle_candidate();
        state.ops.finish = CardFinish::Holo;
        state.ops.condition = CardCondition::LightlyPlayed;
        state.receive_record(record.clone());
        assert_eq!(state.ops.candidate.as_deref(), Some("b"));
        assert_eq!(state.ops.finish, CardFinish::Holo);
        assert_eq!(state.ops.condition, CardCondition::LightlyPlayed);
        let mut saved = record.identifications().unwrap().remove(0);
        saved.reviewed_at = Some(saved.created_at);
        saved.selected_card_id = Some("a".into());
        saved.state = IdentificationState::Confirmed;
        saved.finish = CardFinish::Normal;
        record.replace_identification(&saved).unwrap();
        state.receive_record(record);
        assert_eq!(state.ops.candidate.as_deref(), Some("a"));
        assert_eq!(state.ops.finish, CardFinish::Normal);
    }
    #[test]
    fn review_request_keeps_the_displayed_identification_and_explicit_assessments() {
        let mut state = State::default();
        state.receive_record(record());
        state.client =
            Some(scanner_client::Client::new("http://127.0.0.1:1", &"a".repeat(64)).unwrap());
        let expected = state.ops.review.as_ref().unwrap().request.clone();
        state.cycle_candidate();
        state.ops.finish = CardFinish::ReverseHolo;
        state.ops.condition = CardCondition::NearMint;
        state.confirm_review();
        state.ops.mutation.cancel();
        let pending = state.ops.pending.as_ref().unwrap();
        assert_eq!(pending.method, "images.review");
        let request: ReviewRequest = serde_json::from_value(pending.params.clone()).unwrap();
        assert_eq!(request.identification_id, expected.id);
        assert_eq!(request.job_id, expected.job_id);
        assert_eq!(request.card_id.as_deref(), Some("b"));
        assert_eq!(request.finish, CardFinish::ReverseHolo);
        assert_eq!(request.condition, CardCondition::NearMint);
    }
}
