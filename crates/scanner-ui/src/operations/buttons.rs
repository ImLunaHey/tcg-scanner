use super::Command;
use crate::{connection_view::button, state::State};
use rune::{
    El, col, row, text,
    tok::{self, type_},
};
use serde_json::json;
impl State {
    pub fn observe_button(&mut self) {
        self.submit(Command {
            method: "diagnostics.buttonMonitor",
            params: json!({"seconds":30}),
            job: None,
            focus: false,
        });
    }
    pub fn stop_button_observation(&mut self) {
        self.submit(Command {
            method: "diagnostics.stopButtonMonitor",
            params: json!({}),
            job: None,
            focus: false,
        });
    }
}
pub(super) fn panel(s: &State, t: tok::Theme) -> El {
    let observing = s.status.as_ref().is_some_and(|v| v.button_observation);
    let idle = !s.ops.busy()
        && s.ops.devices.len() == 1
        && s.status.as_ref().is_some_and(|v| {
            !v.paused
                && !v.stopping
                && !v.requires_restart
                && !v.button_observation
                && v.active_job_id.is_none()
                && v.queued_jobs == 0
        });
    col()
        .gap(8.)
        .child(
            text(if observing {
                "Observing scanner button · no pages will be fed"
            } else {
                "Button diagnostics observe driver events without creating a scan."
            })
            .font(type_::LABEL)
            .color(t.text_secondary)
            .wrap(),
        )
        .child(
            row()
                .gap(8.)
                .child(button(
                    "Observe button for 30s",
                    "observe-button",
                    idle,
                    t,
                    State::observe_button,
                ))
                .child(button(
                    "Stop observation",
                    "stop-observation",
                    observing && !s.ops.busy(),
                    t,
                    State::stop_button_observation,
                )),
        )
}
