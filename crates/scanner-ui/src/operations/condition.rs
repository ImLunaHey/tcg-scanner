use super::Command;
use crate::{connection_view::button, state::State};
use rune::{
    El, col, text,
    tok::{self, type_},
};
use scanner_types::{
    cards::CardGame,
    condition::{ConditionCheck, ConditionState},
    game_detection::CardGameDetection,
    identification::IdentificationState,
};

impl State {
    pub fn check_condition(&mut self) {
        let (Some(record), Some(review)) = (&self.record, &self.ops.review) else {
            return;
        };
        self.submit(Command {
            method: "images.checkCondition",
            params: serde_json::json!({"jobID":record.id,"sequence":review.request.sequence,"identificationID":review.request.id}),
            job: Some(record.id), focus: false,
        });
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
    let ready = supported && !s.ops.busy() && review.state != IdentificationState::Processing;
    let mut panel = col().gap(6.).child(button(
        "Check visible marks",
        "check-condition",
        ready,
        t,
        State::check_condition,
    ));
    if let Some(check) = review
        .extensions
        .get("conditionCheck")
        .and_then(|v| serde_json::from_value::<ConditionCheck>(v.clone()).ok())
    {
        let label = match check.state {
            ConditionState::Checked => format!(
                "{} possible border marks on the identified source",
                check.findings.len()
            ),
            ConditionState::Unsupported => "Visible marks: source not supported".into(),
            ConditionState::Inconclusive => "Visible marks: inconclusive".into(),
        };
        panel = panel
            .child(
                text(label)
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap(),
            )
            .child(
                text("Observations only; your condition choice is unchanged.")
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap(),
            );
        if let Some(limitation) = check.limitations.first() {
            panel = panel.child(
                text(limitation)
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .wrap(),
            );
        }
    }
    panel.child(crate::image_review::controls(s, t))
}
