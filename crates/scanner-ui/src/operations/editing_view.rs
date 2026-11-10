use crate::{connection_view::button, state::State};
use rune::{
    El, col, row, text, text_input,
    tok::{self, type_},
};

pub fn panel(s: &State, t: tok::Theme) -> El {
    let Some(editor) = &s.ops.editor else {
        return col();
    };
    let idle = !s.ops.busy();
    let fields = row().gap(8.).children(
        ["Left %", "Top %", "Width %", "Height %"]
            .into_iter()
            .enumerate()
            .map(|(i, label)| {
                let input = if idle {
                    text_input(&editor.crop[i], type_::BODY)
                        .on_edit(move |s: &mut State| &mut s.ops.editor.as_mut().unwrap().crop[i])
                } else {
                    text(&editor.crop[i].text).font(type_::BODY)
                };
                col()
                    .grow(1.)
                    .min_w(0.)
                    .gap(4.)
                    .child(text(label).font(type_::LABEL).color(t.text_secondary))
                    .child(
                        input
                            .key(format!("crop-{i}"))
                            .label(label)
                            .w_full()
                            .h(36.)
                            .px(8.)
                            .border(1., t.card_border)
                            .radius(6.)
                            .color(t.text),
                    )
            }),
    );
    col().gap(8.)
        .child(text("Original shown above. Crop percentages apply after deskew, before clockwise rotation.")
            .font(type_::LABEL).color(t.text_secondary).wrap())
        .child(fields)
        .child(orientation(s, t))
        .child(row().gap(8.)
            .child(button("Save edited copy", "save-edit", idle, t, State::save_edit))
            .child(button("Reset to original", "reset-edit", idle, t, State::reset_edit))
            .child(button("Close editor", "close-edit", idle, t, State::close_edit)))
        .child(text("Each save creates a separate image. Up to eight edits per original.")
            .font(type_::LABEL).color(t.text_secondary).wrap())
}
fn orientation(s: &State, t: tok::Theme) -> El {
    let editor = s.ops.editor.as_ref().unwrap();
    let idle = !s.ops.busy();
    let input = if idle {
        text_input(&editor.deskew, type_::BODY)
            .on_edit(|s: &mut State| &mut s.ops.editor.as_mut().unwrap().deskew)
    } else {
        text(&editor.deskew.text).font(type_::BODY)
    };
    row()
        .gap(8.)
        .center_y()
        .child(
            text("Deskew ° (−12 to 12)")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            input
                .key("deskew")
                .label("Deskew degrees")
                .w(90.)
                .h(36.)
                .px(8.)
                .border(1., t.card_border)
                .radius(6.)
                .color(t.text),
        )
        .child(button(
            &format!("Rotation {}°", u16::from(editor.turns) * 90),
            "rotate-edit",
            idle,
            t,
            |s| {
                if let Some(e) = &mut s.ops.editor {
                    e.turns = (e.turns + 1) % 4;
                }
            },
        ))
}
