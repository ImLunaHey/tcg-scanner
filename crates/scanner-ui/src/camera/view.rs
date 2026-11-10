use crate::{connection_view::button, state::State};
use rune::{
    El, ScrollSpec, SurfaceFit, col, el, row, text,
    tok::{self, type_},
};
use scanner_types::cards::CardLanguage;
pub fn view(s: &State, t: tok::Theme) -> El {
    let c = &s.camera;
    let (width, height) = viewport();
    let width = (width - 32.).clamp(240., 520.);
    if c.details {
        return details(s, t, width);
    }
    let height = (height - 310.).clamp(200., 480.).min(width * 4. / 3.);
    let panel = col()
        .w(width)
        .gap(12.)
        .items_center()
        .child(header(t))
        .child(languages(s, t))
        .child(preview(s, t, height))
        .child(controls(s, t))
        .child(feedback(s, t));
    col()
        .w_full()
        .h_full()
        .py(16.)
        .fill(t.bg)
        .items_center()
        .key("camera-scroll")
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(panel)
}
fn viewport() -> (f32, f32) {
    web_sys::window()
        .map(|w| {
            (
                w.inner_width()
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(390.) as f32,
                w.inner_height()
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(800.) as f32,
            )
        })
        .unwrap_or((390., 800.))
}
fn header(t: tok::Theme) -> El {
    row()
        .w_full()
        .gap(8.)
        .child(
            text("Card camera")
                .font(type_::TITLE)
                .color(t.text)
                .grow(1.),
        )
        .child(
            el().role("link")
                .label("Close camera")
                .href("/")
                .child(text("Close").font(type_::LABEL).color(t.text)),
        )
}
fn languages(s: &State, t: tok::Theme) -> El {
    row().gap(8.).children(
        [
            (CardLanguage::English, "English", "camera-english"),
            (CardLanguage::Korean, "Korean", "camera-korean"),
        ]
        .map(|(language, label, key)| {
            let label = if s.camera.language == language {
                format!("{label} ✓")
            } else {
                label.into()
            };
            button(&label, key, !s.camera.lookup.pending(), t, move |s| {
                s.camera.language(language)
            })
        }),
    )
}
fn preview(s: &State, t: tok::Theme, height: f32) -> El {
    let mut preview = el()
        .w(height * 0.75)
        .h(height)
        .radius(16.)
        .fill(t.card)
        .center()
        .role("img")
        .label("Live camera preview");
    if let Some(image) = &s.camera.live {
        preview = preview.pixels_fit(image.pixels(), SurfaceFit::Fit);
    } else {
        preview = preview.child(note("Hold one card front inside the frame", t));
    }
    preview
}
fn controls(s: &State, t: tok::Theme) -> El {
    let c = &s.camera;
    let mut controls = row().gap(8.).child(button(
        if c.active() {
            "Stop camera"
        } else {
            "Start camera"
        },
        "camera-start",
        true,
        t,
        |s| {
            if s.camera.active() {
                s.camera.stop()
            } else {
                s.camera.start()
            }
        },
    ));
    if c.torch_supported {
        controls = controls.child(button(
            if c.torch_on { "Torch off" } else { "Torch on" },
            "camera-torch",
            !c.torch_task.pending(),
            t,
            |s| s.camera.toggle_torch(),
        ));
    }
    controls
}
fn feedback(s: &State, t: tok::Theme) -> El {
    let c = &s.camera;
    let mut panel = col()
        .w_full()
        .gap(10.)
        .child(
            text(&c.status)
                .font(type_::BODY)
                .wrap()
                .color(t.text)
                .role("status")
                .label(&c.status),
        )
        .child(note("Nothing is saved or added to your collection.", t));
    if let Some(error) = &c.error {
        panel = panel.child(
            text(error)
                .font(type_::LABEL)
                .wrap()
                .color(t.text)
                .role("alert")
                .label(error),
        );
    }
    if let Some(result) = &c.result {
        let title = result_title(result);
        panel = panel.child(
            row()
                .gap(8.)
                .child(button(
                    &format!("View {title} details"),
                    "camera-details",
                    true,
                    t,
                    |s| s.camera.show_details(true),
                ))
                .child(button("Check again", "camera-again", c.active(), t, |s| {
                    s.camera.reset()
                })),
        );
    }
    panel
}
fn details(s: &State, t: tok::Theme, width: f32) -> El {
    let c = &s.camera;
    let mut panel = col()
        .w(width)
        .gap(14.)
        .child(button(
            "Back to camera",
            "camera-close-details",
            true,
            t,
            |s| s.camera.show_details(false),
        ))
        .child(text("Card details").font(type_::TITLE).color(t.text));
    if let Some(image) = &c.preview {
        panel = panel.child(
            el().w(180.)
                .h(240.)
                .pixels_fit(image.pixels(), SurfaceFit::Fit)
                .role("img")
                .label("Last identified card"),
        );
    }
    if let Some(result) = &c.result {
        for candidate in &result.candidates {
            panel = panel
                .child(
                    text(&candidate.name)
                        .font(type_::TITLE)
                        .wrap()
                        .color(t.text),
                )
                .child(
                    text(format!("{} · {}", candidate.set_name, candidate.local_id))
                        .font(type_::BODY)
                        .wrap()
                        .color(t.text),
                );
            for evidence in &candidate.evidence {
                panel = panel.child(note(evidence, t));
            }
        }
        panel = panel.child(lookup_notes(result, t));
        if !result.visual_matches.is_empty() {
            panel=panel.child(note("Similar artwork — check the printed set and number. These are not verified printing matches.",t));
            for visual in &result.visual_matches {
                panel = panel.child(note(
                    &format!(
                        "{} · {} {}",
                        visual.candidate.name, visual.candidate.set_name, visual.candidate.local_id
                    ),
                    t,
                ));
            }
        }
    }
    panel = panel.child(note(
        "Check the physical card. A catalogue suggestion does not confirm its finish or condition.",
        t,
    ));
    col()
        .w_full()
        .h_full()
        .py(20.)
        .fill(t.bg)
        .items_center()
        .key("camera-details-scroll")
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(panel)
}
fn note(value: &str, t: tok::Theme) -> El {
    text(value).font(type_::BODY).wrap().color(t.text_secondary)
}

fn result_title(result: &scanner_types::camera::CameraIdentification) -> String {
    if let Some(card) = result.candidates.first() {
        return card.name.clone();
    }
    if let Some(name) = result
        .uncatalogued_card
        .as_ref()
        .and_then(|v| v["name"].as_str())
    {
        return name.into();
    }
    if let Some(kind) = result
        .energy_symbol
        .as_ref()
        .and_then(|v| v["type"].as_str())
    {
        return format!("{kind} Energy");
    }
    if result.state == scanner_types::camera::CameraState::CodeCard {
        return "Pokémon LIVE code card".into();
    }
    if !result.visual_matches.is_empty() {
        return "similar artwork".into();
    }
    "Last card".into()
}
fn finish_details(finish: &scanner_types::finish::FinishEstimate, t: tok::Theme) -> El {
    use scanner_types::{edition::EditionState, finish::FinishState, recognition::CardFinish};
    let label = match (finish.state, finish.suggested_finish) {
        (FinishState::Estimated, Some(value)) => format!(
            "Suggested finish: {}",
            match value {
                CardFinish::Normal => "normal",
                CardFinish::Holo => "holo",
                CardFinish::ReverseHolo => "reverse holo",
                CardFinish::Other => "other",
                CardFinish::Unverified => "unverified",
            }
        ),
        (FinishState::Unsupported, _) => "Automatic finish is unavailable for this card.".into(),
        _ => "Finish is uncertain. Inspect the physical card.".into(),
    };
    let mut panel = col().gap(8.).child(note(&label, t));
    if let Some(edition) = &finish.edition {
        panel = panel.child(note(
            &match edition.state {
                EditionState::Matched => format!(
                    "Edition: {}",
                    edition.name.as_deref().unwrap_or("stamped printing")
                ),
                EditionState::NotDetected => "No supported stamp established.".into(),
                _ => "Stamped edition remains unresolved.".into(),
            },
            t,
        ));
    }
    for observation in finish.evidence.iter().chain(&finish.limitations) {
        panel = panel.child(note(observation, t));
    }
    panel
}

fn lookup_notes(result: &scanner_types::camera::CameraIdentification, t: tok::Theme) -> El {
    let mut panel = col().gap(8.);
    if let Some(uncatalogued) = &result.uncatalogued_card {
        panel = panel.child(note(
            &format!(
                "{} · {} {}",
                uncatalogued["name"].as_str().unwrap_or("Uncatalogued card"),
                uncatalogued["setCode"].as_str().unwrap_or(""),
                uncatalogued["collectorNumber"].as_str().unwrap_or("")
            ),
            t,
        ));
    }
    if let Some(energy) = &result.energy_symbol {
        panel = panel.child(note(
            &format!("{} Energy", energy["type"].as_str().unwrap_or("Unresolved")),
            t,
        ));
    }
    if result.state == scanner_types::camera::CameraState::CodeCard {
        panel = panel.child(note("Pokémon LIVE code card", t));
    }
    if let Some(finish) = &result.finish_estimate {
        panel = panel.child(finish_details(finish, t));
    }
    panel
}
