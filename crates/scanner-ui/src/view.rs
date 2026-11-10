use crate::state::State;
use rune::{
    Cx, El, ScrollSpec, col, el, row, text, text_input,
    tok::{self, type_},
};

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let t = tok::theme(cx.system_dark());
    #[cfg(target_arch = "wasm32")]
    if s.camera.enabled {
        return crate::camera::view(s, t);
    }
    if s.client.is_none() && !s.fixture {
        return crate::connection_view::login(s, t);
    }
    row()
        .key("scanner")
        .w_full()
        .h_full()
        .fill(t.bg)
        .child(batches(s, t))
        .child(match s.page {
            crate::state::Page::Library => library_view(s, t),
            crate::state::Page::Collection => crate::collection::view(s, t),
            crate::state::Page::Settings => crate::operations::settings_view(s, t),
        })
}
fn library_view(s: &State, t: tok::Theme) -> El {
    col()
        .grow(1.)
        .min_w(0.)
        .h_full()
        .px(24.)
        .py(24.)
        .gap(20.)
        .child(crate::connection_view::toolbar(s, t))
        .child(crate::operations::controls(s, t))
        .child(crate::exports::panel(s, t))
        .child(
            text(
                s.error
                    .as_deref()
                    .unwrap_or("Browse saved scans and identification evidence"),
            )
            .font(type_::BODY)
            .color(t.text_secondary)
            .wrap(),
        )
        .child(
            row()
                .grow(1.)
                .min_h(0.)
                .gap(20.)
                .child(card_list(s, t))
                .child(detail(s, t)),
        )
}

fn batches(s: &State, t: tok::Theme) -> El {
    let base = col()
        .key("batches")
        .w(206.)
        .h_full()
        .px(16.)
        .py(24.)
        .gap(12.)
        .fill(t.card)
        .child(text("Scanner").font(type_::TITLE).color(t.text))
        .children(crate::camera::entry(t))
        .child(crate::connection_view::button(
            "Batches",
            "nav-batches",
            true,
            t,
            |s| s.page = crate::state::Page::Library,
        ))
        .child(crate::connection_view::button(
            "Inventory",
            "nav-inventory",
            !s.fixture,
            t,
            State::show_collection,
        ))
        .child(crate::connection_view::button(
            "Settings",
            "nav-settings",
            !s.fixture,
            t,
            State::show_settings,
        ));
    if s.page == crate::state::Page::Settings {
        return base.child(crate::operations::settings_sidebar(s, t));
    }
    if s.page == crate::state::Page::Collection {
        return base.child(crate::collection::sidebar(s, t));
    }
    base.child(batch_history(s, t))
}
fn batch_history(s: &State, t: tok::Theme) -> El {
    col()
        .grow(1.)
        .min_h(0.)
        .gap(12.)
        .child(
            text(format!("{} saved batches", s.batches.len()))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            col()
                .grow(1.)
                .min_h(0.)
                .w_full()
                .gap(6.)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .children(s.batches.iter().enumerate().map(|(i, batch)| {
                    col()
                        .key(format!("batch-{i}"))
                        .w_full()
                        .px(10.)
                        .py(12.)
                        .gap(5.)
                        .radius(8.)
                        .fill(if s.batch == i {
                            t.control_track
                        } else {
                            t.card
                        })
                        .role("button")
                        .label(batch_label(batch))
                        .focusable(true)
                        .on_press(move |s: &mut State| s.select_batch(i))
                        .child(text(batch_label(batch)).font(type_::LABEL).color(t.text))
                        .child(
                            text(format!(
                                "{} images · {}",
                                batch.progress.images_saved,
                                batch.state.as_str()
                            ))
                            .font(type_::LABEL)
                            .color(t.text_secondary),
                        )
                })),
        )
}

fn card_list(s: &State, t: tok::Theme) -> El {
    let query = s.query.text.to_lowercase();
    col()
        .w(280.)
        .h_full()
        .gap(12.)
        .child(
            el().w_full()
                .h(36.)
                .px(10.)
                .center_y()
                .radius(8.)
                .fill(t.card)
                .border(1., t.card_border)
                .child(
                    text_input(&s.query, type_::BODY)
                        .w_full()
                        .key("search")
                        .label("Search cards")
                        .placeholder("Search cards…", t.text_secondary)
                        .color(t.text)
                        .on_edit(|s: &mut State| &mut s.query),
                ),
        )
        .child(
            col()
                .key("cards")
                .grow(1.)
                .min_h(0.)
                .w_full()
                .gap(6.)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .children(
                    s.cards
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| {
                            query.is_empty()
                                || format!("{} {} {}", c.name, c.subtitle, c.sequence)
                                    .to_lowercase()
                                    .contains(&query)
                        })
                        .map(|(i, card)| {
                            col()
                                .key(format!("card-{i}"))
                                .w_full()
                                .px(12.)
                                .py(12.)
                                .gap(5.)
                                .radius(8.)
                                .fill(if s.selected == i {
                                    t.control_track
                                } else {
                                    t.card
                                })
                                .role("button")
                                .label(&card.name)
                                .focusable(true)
                                .on_press(move |s: &mut State| s.select_card(i))
                                .child(
                                    text(format!("{:03}  {}", card.sequence, card.name))
                                        .font(type_::LABEL)
                                        .color(t.text)
                                        .wrap(),
                                )
                                .child(
                                    text(&card.subtitle)
                                        .font(type_::LABEL)
                                        .color(t.text_secondary)
                                        .wrap(),
                                )
                        }),
                ),
        )
}

fn detail(s: &State, t: tok::Theme) -> El {
    let mut pane = col().key("detail").grow(1.).min_w(0.).h_full().gap(12.);
    let Some(card) = s.cards.get(s.selected) else {
        return pane.child(
            text("Select a saved batch to see its cards.")
                .font(type_::BODY)
                .color(t.text_secondary)
                .wrap(),
        );
    };
    pane = pane
        .child(text(&card.name).font(type_::TITLE).color(t.text))
        .child(
            text(format!("{} · {}", card.subtitle, card.status))
                .font(type_::BODY)
                .color(t.text_secondary)
                .wrap(),
        );
    let preview = crate::image_review::preview(s, t);
    if s.ops.editor.is_some() {
        return pane
            .child(preview)
            .child(crate::operations::editing_panel(s, t));
    }
    let mut images = row().grow(1.).min_h(120.).gap(12.).child(preview);
    if s.comparison.active() {
        images = images.child(crate::comparison::view(s, t));
    }
    pane.child(crate::comparison::control(s, t))
        .child(images)
        .child(
            col()
                .key("review-scroll")
                .h(340.)
                .min_h(0.)
                .w_full()
                .gap(12.)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(crate::operations::review_panel(s, t))
                .child(crate::valuation::panel(s, t))
                .child(crate::operations::staging_panel(s, t))
                .child(
                    col().gap(6.).children(
                        card.evidence.iter().take(6).map(|line| {
                            text(line).font(type_::BODY).color(t.text_secondary).wrap()
                        }),
                    ),
                ),
        )
}

fn batch_label(batch: &scanner_types::service::JobSummary) -> String {
    batch
        .batch_number
        .map(|n| format!("B-{n:03}"))
        .unwrap_or_else(|| batch.id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use scanner_types::{
        device::Device,
        scan::{ScanRecord, ScanRequest},
    };
    #[test]
    fn selecting_another_batch_updates_the_library() {
        let mut state = State::default();
        state.fixture = true;
        for number in 1..=2 {
            let mut batch = ScanRecord::new(
                Device::default(),
                ScanRequest::default(),
                "/unavailable".into(),
            );
            batch.batch_number = Some(number);
            state.batches.push((&batch).into());
        }
        let mut rt = rune::Runtime::new(
            state,
            view,
            (1180., 780.),
            rune::Env {
                system_dark: true,
                scale: 1.,
            },
        );
        rt.frame(0.);
        let (x, y) = rt.find("batch-1").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.016);
        assert_eq!(rt.state.batch, 1);
    }
}
