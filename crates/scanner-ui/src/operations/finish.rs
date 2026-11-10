use super::Command;
use crate::{connection_view::button, state::State};
use rune::{
    El, col, text,
    tok::{self, type_},
};
use scanner_types::{
    cards::CardGame,
    edition::EditionState,
    finish::{FinishEstimate, FinishState},
    game_detection::CardGameDetection,
    identification::IdentificationState,
};
impl State {
    pub fn estimate_finish(&mut self) {
        let (Some(record), Some(review)) = (&self.record, &self.ops.review) else {
            return;
        };
        self.submit(Command { method:"images.estimateFinish",job:Some(record.id),focus:false,
            params:serde_json::json!({"jobID":record.id,"sequence":review.request.sequence,"identificationID":review.request.id}) });
    }
}
pub(super) fn panel(s: &State, t: tok::Theme) -> El {
    let Some(review) = &s.ops.review else {
        return col();
    };
    let supported = s.record.as_ref().is_some_and(|r| {
        CardGameDetection::resolve(r)
            .iter()
            .any(|p| p.number == review.request.sequence.div_ceil(2) && p.game == CardGame::Pokemon)
    });
    let ready = supported
        && !s.ops.busy()
        && review.state != IdentificationState::Processing
        && review.evidence.is_some();
    let mut panel = col().gap(6.).child(button(
        "Assess finish",
        "estimate-finish",
        ready,
        t,
        State::estimate_finish,
    ));
    let Some(estimate) = review
        .extensions
        .get("finishEstimate")
        .and_then(|v| serde_json::from_value::<FinishEstimate>(v.clone()).ok())
    else {
        return panel;
    };
    let label = match (estimate.state, estimate.suggested_finish) {
        (FinishState::Estimated, Some(finish)) => {
            format!("Suggested finish: {}", super::view::finish_label(finish))
        }
        (FinishState::Unsupported, _) => "Automatic finish: source not supported".into(),
        _ => "Automatic finish: uncertain".into(),
    };
    panel = panel.child(note(label, t));
    if let Some(edition) = &estimate.edition {
        let label = match edition.state {
            EditionState::Matched => format!(
                "Edition: {}",
                edition.name.as_deref().unwrap_or("stamped printing")
            ),
            EditionState::NotDetected => "No supported stamp established".into(),
            _ => "Stamped edition: unresolved".into(),
        };
        panel = panel.child(note(label, t));
    }
    for evidence in estimate.evidence.iter().take(2) {
        panel = panel.child(note(evidence.clone(), t));
    }
    panel.child(note(
        "Inspect the physical card before choosing its finish.".into(),
        t,
    ))
}
fn note(value: String, t: tok::Theme) -> El {
    text(value)
        .font(type_::LABEL)
        .color(t.text_secondary)
        .wrap()
}
