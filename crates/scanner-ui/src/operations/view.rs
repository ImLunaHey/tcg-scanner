use crate::{connection_view::button, state::State};
use rune::{
    El, col, row, text,
    tok::{self, type_},
};
use scanner_types::{
    cards::CardLanguage,
    identification::IdentificationState,
    recognition::{CardCondition, CardFinish},
    scan::ScanState,
};

pub fn controls(s: &State, t: tok::Theme) -> El {
    if s.fixture {
        return col();
    }
    col()
        .gap(8.)
        .child(selectors(s, t))
        .child(scan_buttons(s, t))
        .child(button(
            if s.status.as_ref().is_some_and(|s| s.paused) {
                "Resume service"
            } else {
                "Pause and cancel scans"
            },
            "pause-service",
            !s.ops.busy() && s.status.as_ref().is_some_and(|s| !s.stopping),
            t,
            State::toggle_pause,
        ))
        .child(retry(s, t))
}
fn selectors(s: &State, t: tok::Theme) -> El {
    let available = !s.ops.busy();
    let device = s
        .ops
        .devices
        .get(s.ops.device)
        .map(|d| d.name.as_str())
        .unwrap_or("No scanner found");
    let profile = s.ops.profile.and_then(|i| s.ops.profiles.get(i));
    let profile_name = if s.ops.use_defaults {
        "Device defaults"
    } else {
        profile.map(|p| p.name.as_str()).unwrap_or("600 DPI · PNG")
    };
    let duplex = profile.map(|p| p.settings.duplex).unwrap_or(s.ops.duplex);
    row()
        .gap(8.)
        .child(button(
            device,
            "choose-device",
            available && s.ops.devices.len() > 1,
            t,
            |s| s.ops.device = (s.ops.device + 1) % s.ops.devices.len(),
        ))
        .child(button(
            profile_name,
            "choose-profile",
            available,
            t,
            cycle_profile,
        ))
        .child(button(
            if s.ops.use_defaults {
                "Sides from defaults"
            } else if duplex {
                "Duplex"
            } else {
                "Single side"
            },
            "duplex",
            available && profile.is_none() && !s.ops.use_defaults,
            t,
            |s| s.ops.duplex = !s.ops.duplex,
        ))
        .child(button(
            if s.ops.language == CardLanguage::English {
                "English"
            } else {
                "Korean"
            },
            "language",
            available,
            t,
            cycle_language,
        ))
}
fn cycle_profile(s: &mut State) {
    if s.ops.use_defaults {
        s.ops.use_defaults = false;
        s.ops.profile = (!s.ops.profiles.is_empty()).then_some(0);
    } else if let Some(index) = s.ops.profile {
        s.ops.profile = (index + 1 < s.ops.profiles.len()).then_some(index + 1);
    } else {
        s.ops.use_defaults = true;
    }
}
fn cycle_language(s: &mut State) {
    s.ops.language = if s.ops.language == CardLanguage::English {
        CardLanguage::Korean
    } else {
        CardLanguage::English
    };
}
fn status_text(s: &State) -> String {
    s.status
        .as_ref()
        .map(|s| {
            if s.stopping {
                "Service stopping".into()
            } else if s.button_observation {
                "Observing scanner button".into()
            } else if s.paused {
                "Scanner paused".into()
            } else {
                format!(
                    "{} queued · {}",
                    s.queued_jobs,
                    if s.active_job_id.is_some() {
                        "Scanning"
                    } else {
                        "Ready"
                    }
                )
            }
        })
        .unwrap_or_else(|| "Connecting…".into())
}
fn scan_buttons(s: &State, t: tok::Theme) -> El {
    let available = !s.ops.busy();
    let can_start = available
        && !s.ops.devices.is_empty()
        && s.status.as_ref().is_some_and(|s| {
            !s.stopping
                && !s.paused
                && !s.button_observation
                && !s.requires_restart
                && s.persistence_error.is_none()
        });
    row()
        .gap(8.)
        .center_y()
        .child(button(
            "Scan cards",
            "scan",
            can_start,
            t,
            State::start_scan,
        ))
        .child(button(
            "Find scanners",
            "discover",
            !s.ops.discovery.pending(),
            t,
            State::discover,
        ))
        .child(button(
            "Cancel batch",
            "cancel-scan",
            available && s.record.as_ref().is_some_and(|r| !r.state.terminal()),
            t,
            State::cancel_scan,
        ))
        .child(button(
            "Continue batch",
            "continue",
            available
                && s.record
                    .as_ref()
                    .is_some_and(|r| r.state == ScanState::Completed && r.session_closed),
            t,
            State::continue_batch,
        ))
        .child(
            text(status_text(s))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
}
pub(super) fn retry(s: &State, t: tok::Theme) -> El {
    if s.ops.mutation.pending() {
        return text("Saving request…")
            .font(type_::LABEL)
            .color(t.text_secondary);
    }
    if s.ops.pending.is_some() {
        return button(
            "Retry same request",
            "retry-command",
            true,
            t,
            State::retry_command,
        );
    }
    col()
}
pub fn review_panel(s: &State, t: tok::Theme) -> El {
    if s.fixture || s.record.is_none() || s.cards.is_empty() {
        return col();
    }
    let idle = !s.ops.busy() && s.record.as_ref().is_some_and(|r| r.state.terminal());
    let mut panel = col().gap(8.).child(
        row()
            .gap(8.)
            .child(button(
                "Prepare card",
                "prepare",
                idle,
                t,
                State::prepare_card,
            ))
            .child(button(
                "Identify card",
                "identify",
                idle,
                t,
                State::identify_card,
            ))
            .child(button(
                "Edit image",
                "edit-image",
                idle,
                t,
                State::begin_edit,
            )),
    );
    let Some(review) = &s.ops.review else {
        return panel;
    };
    let ready = idle && review.state != IdentificationState::Processing;
    let candidates = review
        .evidence
        .as_ref()
        .map(|e| e.review_candidates())
        .unwrap_or_default();
    let selected = candidates
        .iter()
        .find(|c| Some(&c.id) == s.ops.candidate.as_ref());
    let label = selected
        .map(|c| format!("{} · {} {}", c.name, c.set_name, c.local_id))
        .unwrap_or_else(|| {
            if candidates.is_empty() {
                "No candidate"
            } else {
                "Choose similar artwork"
            }
            .into()
        });
    panel = panel.child(button(
        &label,
        "candidate",
        ready && (candidates.len() > 1 || (selected.is_none() && !candidates.is_empty())),
        t,
        State::cycle_candidate,
    ));
    if let Some(selected) = selected {
        for evidence in &selected.evidence {
            panel = panel.child(
                text(evidence)
                    .font(type_::BODY)
                    .wrap()
                    .color(t.text_secondary),
            );
        }
    }
    panel
        .child(assessment(s, t, ready))
        .child(review_buttons(t, ready, selected.is_some()))
        .child(super::finish::panel(s, t))
        .child(super::condition::panel(s, t))
}
fn assessment(s: &State, t: tok::Theme, ready: bool) -> El {
    row()
        .gap(8.)
        .child(button(
            finish_label(s.ops.finish),
            "finish",
            ready,
            t,
            State::cycle_finish,
        ))
        .child(button(
            condition_label(s.ops.condition),
            "condition",
            ready,
            t,
            State::cycle_condition,
        ))
}
fn review_buttons(t: tok::Theme, ready: bool, selected: bool) -> El {
    row()
        .gap(8.)
        .child(button(
            "Confirm card",
            "confirm",
            ready && selected,
            t,
            State::confirm_review,
        ))
        .child(button(
            "Reject match",
            "reject",
            ready,
            t,
            State::reject_review,
        ))
}

pub(super) fn finish_label(value: CardFinish) -> &'static str {
    use CardFinish::*;
    match value {
        Unverified => "Finish unverified",
        Normal => "Normal",
        Holo => "Holo",
        ReverseHolo => "Reverse holo",
        Other => "Other finish",
    }
}
fn condition_label(value: CardCondition) -> &'static str {
    use CardCondition::*;
    match value {
        Unassessed => "Condition unassessed",
        NearMint => "Near mint",
        LightlyPlayed => "Lightly played",
        ModeratelyPlayed => "Moderately played",
        HeavilyPlayed => "Heavily played",
        Damaged => "Damaged",
    }
}
