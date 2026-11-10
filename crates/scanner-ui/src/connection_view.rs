use crate::state::State;
use rune::{
    El, col, el, row, text, text_input,
    tok::{self, type_},
};

pub fn button(
    label: &str,
    key: &str,
    enabled: bool,
    t: tok::Theme,
    action: impl Fn(&mut State) + 'static,
) -> El {
    let button = el()
        .key(key)
        .min_w(0.)
        .px(12.)
        .py(9.)
        .radius(8.)
        .fill(t.control_track)
        .child(text(label).font(type_::LABEL).wrap().color(if enabled {
            t.text
        } else {
            t.text_secondary
        }));
    if enabled {
        button
            .role("button")
            .label(label)
            .focusable(true)
            .on_press(action)
    } else {
        button
    }
}
pub fn login(s: &State, t: tok::Theme) -> El {
    col().w_full().h_full().center().fill(t.bg).child(
        col()
            .w(460.)
            .gap(18.)
            .px(28.)
            .py(28.)
            .radius(16.)
            .fill(t.card)
            .child(text("Connect to Scanner").font(type_::TITLE).color(t.text))
            .child(
                text("Connect to your scanner service to browse saved cards and manage scans.")
                    .font(type_::BODY)
                    .color(t.text_secondary)
                    .wrap(),
            )
            .child(text("Service address").font(type_::LABEL).color(t.text))
            .child(field(
                text_input(&s.origin, type_::BODY)
                    .w_full()
                    .key("service-origin")
                    .label("Service address")
                    .color(t.text)
                    .on_edit(|s: &mut State| &mut s.origin),
                t,
            ))
            .child(text("Access token").font(type_::LABEL).color(t.text))
            .child(field(
                text_input(&s.token, type_::BODY)
                    .w_full()
                    .key("service-token")
                    .label("Access token")
                    .color(t.text)
                    .on_edit(|s: &mut State| &mut s.token),
                t,
            ))
            .child(
                text(
                    s.error
                        .as_deref()
                        .unwrap_or("The token is kept only for this connection."),
                )
                .font(type_::BODY)
                .color(t.text_secondary)
                .wrap(),
            )
            .child(button(
                if s.connecting.pending() {
                    "Connecting…"
                } else {
                    "Connect"
                },
                "connect",
                !s.connecting.pending(),
                t,
                State::connect,
            )),
    )
}
fn field(child: El, t: tok::Theme) -> El {
    child
        .w_full()
        .h(40.)
        .px(10.)
        .center_y()
        .radius(8.)
        .border(1., t.card_border)
}

pub fn toolbar(s: &State, t: tok::Theme) -> El {
    let mut bar = row()
        .gap(10.)
        .center_y()
        .child(text("Card library").font(type_::TITLE).color(t.text));
    if s.fixture {
        return bar.child(text("Saved fixture · read only").color(t.text_secondary));
    }
    bar = bar
        .child(button("Refresh", "refresh", !s.listing.pending(), t, |s| {
            s.refresh(s.offset)
        }))
        .child(button(
            "Previous",
            "previous",
            s.offset > 0 && !s.listing.pending(),
            t,
            |s| s.refresh(s.offset.saturating_sub(100)),
        ))
        .child(button(
            "Next",
            "next",
            s.next_offset.is_some() && !s.listing.pending(),
            t,
            |s| {
                if let Some(offset) = s.next_offset {
                    s.refresh(offset);
                }
            },
        ))
        .child(button(
            "Disconnect",
            "disconnect",
            !s.ops.mutation.pending(),
            t,
            State::disconnect,
        ));
    bar
}
