use crate::{connection_view::button, state::State};
use rune::{
    El, ScrollSpec, SurfaceFit, col, el, row, text, text_input,
    tok::{self, type_},
};

pub fn sidebar(s: &State, t: tok::Theme) -> El {
    let mut list = col()
        .w_full()
        .gap(8.)
        .grow(1.)
        .min_h(0.)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(button("All sets", "all-sets", true, t, |s| {
            s.collection_set(String::new())
        }));
    if let Some(page) = &s.collection.page {
        for set in page.sets.iter().skip(s.collection.set_offset).take(100) {
            let id = set.id.clone();
            let label = format!("{} · {}", set.name, set.quantity);
            list = list.child(button(
                &label,
                &format!("set-{}", set.id),
                true,
                t,
                move |s| s.collection_set(id.clone()),
            ));
        }
        if s.collection.set_offset > 0 {
            list = list.child(button("Previous sets", "previous-sets", true, t, |s| {
                s.collection.set_offset = s.collection.set_offset.saturating_sub(100)
            }));
        }
        if page.sets.len() > s.collection.set_offset + 100 {
            list = list.child(button("More sets", "more-sets", true, t, |s| {
                s.collection.set_offset += 100
            }));
        }
    }
    list
}
pub fn view(s: &State, t: tok::Theme) -> El {
    let c = &s.collection;
    let summary = c
        .page
        .as_ref()
        .map(|p| {
            format!(
                "{} physical copies · {} catalogued · {} awaiting catalogue · {} unresolved · {} code inserts excluded",
                p.total_copies, p.catalogue_copies, p.awaiting_catalogue_copies, p.unresolved_copies, p.excluded_code_cards
            )
        })
        .unwrap_or_else(|| "Loading inventory…".into());
    col()
        .grow(1.)
        .min_w(0.)
        .h_full()
        .padding(24.)
        .gap(16.)
        .child(text("Inventory").font(type_::TITLE).color(t.text))
        .child(
            text(summary)
                .font(type_::BODY)
                .color(t.text_secondary)
                .wrap(),
        )
        .child(toolbar(s, t))
        .child(
            text(
                c.error
                    .as_deref()
                    .or_else(|| {
                        c.missing
                            .as_ref()
                            .filter(|_| c.missing_mode)
                            .and_then(|m| m.note.as_deref())
                    })
                    .unwrap_or(""),
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
                .child(entries(s, t))
                .child(detail(s, t)),
        )
}
fn toolbar(s: &State, t: tok::Theme) -> El {
    let c = &s.collection;
    let previous = if c.missing_mode {
        c.missing_offset > 0
    } else {
        c.offset > 0
    };
    let next = if c.missing_mode {
        c.missing
            .as_ref()
            .is_some_and(|m| m.cards.len() > c.missing_offset + 100)
    } else {
        c.page.as_ref().is_some_and(|p| p.next_offset.is_some())
    };
    row()
        .gap(8.)
        .center_y()
        .child(
            text_input(&c.query, type_::BODY)
                .key("inventory-search")
                .label("Search inventory")
                .placeholder("Search inventory…", t.text_secondary)
                .color(t.text)
                .w(240.)
                .h(38.)
                .px(10.)
                .fill(t.card)
                .radius(8.)
                .on_edit(|s: &mut State| &mut s.collection.query)
                .on_input_change(|s: &mut State, _| {
                    s.collection.missing_mode = false;
                    s.collection_load(0, true);
                }),
        )
        .child(button(
            "Refresh",
            "refresh-inventory",
            !c.listing.pending(),
            t,
            |s| s.collection_load(s.collection.offset, false),
        ))
        .child(button(
            if c.missing_mode {
                "Owned cards"
            } else {
                "Missing cards"
            },
            "missing-cards",
            !c.set.is_empty() && !c.missing_task.pending(),
            t,
            |s| {
                if s.collection.missing_mode {
                    s.collection.missing_mode = false;
                    s.collection_select(0);
                } else {
                    s.collection_missing();
                }
            },
        ))
        .child(button(
            "Previous",
            "inventory-previous",
            previous && !c.listing.pending(),
            t,
            |s| page(s, false),
        ))
        .child(button(
            "Next",
            "inventory-next",
            next && !c.listing.pending(),
            t,
            |s| page(s, true),
        ))
}
fn page(s: &mut State, next: bool) {
    if s.collection.missing_mode {
        s.collection.missing_offset = if next {
            s.collection.missing_offset + 100
        } else {
            s.collection.missing_offset.saturating_sub(100)
        };
        s.collection_select(s.collection.missing_offset);
    } else {
        let offset = if next {
            s.collection
                .page
                .as_ref()
                .and_then(|p| p.next_offset)
                .unwrap_or(s.collection.offset)
        } else {
            s.collection.offset.saturating_sub(100)
        };
        s.collection_load(offset, false);
    }
}
fn entries(s: &State, t: tok::Theme) -> El {
    let c = &s.collection;
    let mut list = col().w(300.).h_full().gap(8.).scroll(ScrollSpec {
        stick_bottom: false,
    });
    if c.missing_mode {
        if let Some(missing) = &c.missing {
            for (i, card) in missing
                .cards
                .iter()
                .enumerate()
                .skip(c.missing_offset)
                .take(100)
            {
                list = list.child(item(
                    s,
                    t,
                    i,
                    &card.name,
                    &format!("{} · {}", card.number, card.language),
                ));
            }
            list = list.child(
                text(format!("{} missing cards", missing.cards.len()))
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            );
        }
    } else if let Some(page) = &c.page {
        for (i, entry) in page.entries.iter().enumerate() {
            list = list.child(item(
                s,
                t,
                i,
                &entry.name,
                &format!(
                    "{} copies · {} reviewed · {}",
                    entry.quantity, entry.confirmed_quantity, entry.language
                ),
            ));
        }
        list = list.child(
            text(format!("{} matching entries", page.matching_entries))
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    list
}
fn item(s: &State, t: tok::Theme, index: usize, name: &str, subtitle: &str) -> El {
    col()
        .key(format!("inventory-card-{index}"))
        .w_full()
        .padding(12.)
        .gap(6.)
        .radius(8.)
        .fill(if s.collection.selected == index {
            t.control_track
        } else {
            t.card
        })
        .role("button")
        .label(name)
        .focusable(true)
        .on_press(move |s: &mut State| s.collection_select(index))
        .child(text(name).font(type_::LABEL).color(t.text).wrap())
        .child(
            text(subtitle)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap(),
        )
}
fn detail(s: &State, t: tok::Theme) -> El {
    let c = &s.collection;
    let mut pane = col().grow(1.).min_w(0.).h_full().gap(12.);
    let title = if c.missing_mode {
        c.missing
            .as_ref()
            .and_then(|m| m.cards.get(c.selected))
            .map(|v| v.name.as_str())
    } else {
        c.page
            .as_ref()
            .and_then(|p| p.entries.get(c.selected))
            .map(|v| v.name.as_str())
    };
    pane = pane.child(
        text(title.unwrap_or("Select a card"))
            .font(type_::TITLE)
            .color(t.text)
            .wrap(),
    );
    let mut preview = el().grow(1.).min_h(120.).w_full().radius(12.).fill(t.card);
    if let Some(image) = &c.image {
        preview = preview.pixels_fit(image.pixels(), SurfaceFit::Fit);
    } else {
        preview = preview.center().child(
            text(if c.preview.pending() {
                "Loading image…"
            } else {
                c.image_error.as_deref().unwrap_or("No preview available")
            })
            .color(t.text_secondary)
            .wrap(),
        );
    }
    pane = pane.child(preview);
    if !c.missing_mode
        && let Some(entry) = c.page.as_ref().and_then(|p| p.entries.get(c.selected))
    {
        pane = pane.child(
            text(format!(
                "{} · {} · {}",
                entry.set_name.as_deref().unwrap_or("Unknown set"),
                entry.number.as_deref().unwrap_or(""),
                entry.status
            ))
            .font(type_::BODY)
            .color(t.text_secondary),
        );
        pane = pane.child(copies(s, t, &entry.copies));
    }
    pane
}

fn copies(s: &State, t: tok::Theme, copies: &[scanner_types::inventory::InventoryCopy]) -> El {
    let c = &s.collection;
    let mut list = col().h(150.).gap(6.).scroll(ScrollSpec {
        stick_bottom: false,
    });
    for copy in copies.iter().skip(c.copy_offset).take(5) {
        let id = copy.batch_id;
        let label = format!(
            "Open batch {} · card {}",
            copy.batch_number
                .map(|n| n.to_string())
                .unwrap_or_else(|| "?".into()),
            copy.card_number
        );
        list = list.child(button(
            &label,
            &format!("copy-{}-{}", id, copy.card_number),
            !c.opening.pending(),
            t,
            move |s| s.collection_open_batch(id),
        ));
    }
    list.child(
        row()
            .gap(8.)
            .child(button(
                "Previous copies",
                "previous-copies",
                c.copy_offset > 0,
                t,
                |s| s.collection.copy_offset = s.collection.copy_offset.saturating_sub(5),
            ))
            .child(button(
                "More copies",
                "more-copies",
                copies.len() > c.copy_offset + 5,
                t,
                |s| s.collection.copy_offset += 5,
            )),
    )
}
