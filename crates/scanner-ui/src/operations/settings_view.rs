use crate::{connection_view::button, state::State};
use rune::{
    El, ScrollSpec, col, row, text, text_input,
    tok::{self, type_},
};
use scanner_types::scan::OriginalFormat;

pub fn sidebar(s: &State, t: tok::Theme) -> El {
    let mut list = col()
        .grow(1.)
        .min_h(0.)
        .w_full()
        .gap(8.)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(button(
            "New profile",
            "new-profile",
            !s.ops.busy(),
            t,
            State::new_profile,
        ));
    for (i, profile) in s
        .ops
        .profiles
        .iter()
        .enumerate()
        .skip(s.ops.settings.profile_offset)
        .take(100)
    {
        list = list.child(button(
            &profile.name,
            &format!("profile-{}", profile.id),
            !s.ops.busy(),
            t,
            move |s| s.edit_profile(i),
        ));
    }
    if s.ops.settings.profile_offset > 0 {
        list = list.child(button(
            "Previous profiles",
            "previous-profiles",
            true,
            t,
            |s| s.ops.settings.profile_offset = s.ops.settings.profile_offset.saturating_sub(100),
        ));
    }
    if s.ops.profiles.len() > s.ops.settings.profile_offset + 100 {
        list = list.child(button("More profiles", "more-profiles", true, t, |s| {
            s.ops.settings.profile_offset += 100
        }));
    }
    list
}
pub fn view(s: &State, t: tok::Theme) -> El {
    let settings = &s.ops.settings;
    col()
        .grow(1.)
        .min_w(0.)
        .h_full()
        .padding(24.)
        .gap(18.)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(text("Scanner settings").font(type_::TITLE).color(t.text))
        .child(
            text(s.error.as_deref().or(settings.notice.as_deref()).unwrap_or(
                "Create reusable scan profiles or save defaults for the selected scanner.",
            ))
            .font(type_::BODY)
            .color(t.text_secondary)
            .wrap(),
        )
        .child(profile_form(s, t))
        .child(super::view::retry(s, t))
        .child(device(s, t))
        .child(
            text(
                s.status
                    .as_ref()
                    .map(|s| format!("Scans saved to {}", s.storage_root))
                    .unwrap_or_default(),
            )
            .font(type_::BODY)
            .color(t.text_secondary)
            .wrap(),
        )
}
fn profile_form(s: &State, t: tok::Theme) -> El {
    let settings = &s.ops.settings;
    let idle = !s.ops.busy();
    col()
        .w(460.)
        .gap(12.)
        .padding(20.)
        .radius(12.)
        .fill(t.card)
        .child(
            text(if settings.id.is_some() {
                "Edit profile"
            } else {
                "New profile"
            })
            .font(type_::TITLE)
            .color(t.text),
        )
        .child(text("Profile name").font(type_::LABEL).color(t.text))
        .child(field(
            &settings.name,
            "profile-name",
            "Profile name",
            !idle,
            t,
            |s| &mut s.ops.settings.name,
        ))
        .child(text("DPI").font(type_::LABEL).color(t.text))
        .child(field(&settings.dpi, "profile-dpi", "DPI", !idle, t, |s| {
            &mut s.ops.settings.dpi
        }))
        .child(format_controls(s, t))
        .child(
            row()
                .gap(8.)
                .child(button(
                    "Save profile",
                    "save-profile",
                    idle,
                    t,
                    State::save_profile,
                ))
                .child(button(
                    "Delete profile",
                    "delete-profile",
                    idle && settings.id.is_some(),
                    t,
                    State::delete_profile,
                )),
        )
}
fn format_controls(s: &State, t: tok::Theme) -> El {
    let settings = &s.ops.settings;
    let idle = !s.ops.busy();
    col().child(
        row()
            .gap(8.)
            .child(button(
                if settings.duplex {
                    "Duplex"
                } else {
                    "Single side"
                },
                "profile-duplex",
                idle,
                t,
                |s| s.ops.settings.duplex = !s.ops.settings.duplex,
            ))
            .child(button(
                if settings.format == OriginalFormat::Png {
                    "PNG"
                } else {
                    "TIFF"
                },
                "profile-format",
                idle,
                t,
                |s| {
                    s.ops.settings.format = if s.ops.settings.format == OriginalFormat::Png {
                        OriginalFormat::Tiff
                    } else {
                        OriginalFormat::Png
                    }
                },
            )),
    )
}

fn field(
    edit: &rune::TextEdit,
    key: &str,
    label: &str,
    busy: bool,
    t: tok::Theme,
    handler: fn(&mut State) -> &mut rune::TextEdit,
) -> El {
    let value = if busy {
        text(&edit.text).font(type_::BODY)
    } else {
        text_input(edit, type_::BODY).on_edit(handler)
    };
    value
        .key(key)
        .label(label)
        .w_full()
        .h(40.)
        .px(10.)
        .border(1., t.card_border)
        .radius(8.)
        .color(t.text)
}

fn device(s: &State, t: tok::Theme) -> El {
    let settings = &s.ops.settings;
    let mut pane = col()
        .gap(12.)
        .child(text("Selected scanner").font(type_::TITLE).color(t.text))
        .child(
            text(
                s.ops
                    .devices
                    .get(s.ops.device)
                    .map(|d| d.name.as_str())
                    .unwrap_or("No scanner found"),
            )
            .font(type_::BODY)
            .color(t.text_secondary),
        )
        .child(
            row()
                .gap(8.)
                .child(button(
                    "Inspect scanner",
                    "inspect-scanner",
                    !settings.inspection.pending() && !s.ops.devices.is_empty(),
                    t,
                    State::inspect_device,
                ))
                .child(button(
                    "Save device defaults",
                    "save-defaults",
                    !s.ops.busy() && !s.ops.devices.is_empty(),
                    t,
                    State::save_device_defaults,
                ))
                .child(button(
                    "Reload profiles and devices",
                    "reload-settings",
                    !s.ops.discovery.pending(),
                    t,
                    State::discover,
                )),
        );
    if let Some(report) = &settings.report {
        let ready = match report.ready {
            Some(true) => "Ready",
            Some(false) => "Not ready",
            None => "Readiness unknown",
        };
        pane = pane.child(
            text(format!("{} · {ready}", report.device.name))
                .font(type_::BODY)
                .color(t.text),
        );
        if let Some(defaults) = &report.configured_settings {
            pane = pane.child(
                text(format!(
                    "Saved defaults: {} DPI · {} · {}",
                    defaults.dpi,
                    if defaults.duplex {
                        "duplex"
                    } else {
                        "single side"
                    },
                    defaults.format.extension()
                ))
                .font(type_::BODY)
                .color(t.text_secondary),
            );
        }
        pane = pane.children(
            report
                .warnings
                .iter()
                .take(6)
                .map(|w| text(w).font(type_::BODY).color(t.text_secondary).wrap()),
        );
    }
    pane
}
