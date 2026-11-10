mod geometry;
use crate::{connection_view::button, state::State};
use rune::{
    El, SurfaceFit, col, el, text,
    tok::{self, type_},
};
use scanner_types::{condition::ConditionCheck, images::ImageCrop};

#[derive(Default)]
pub struct ImageReview {
    pub frame: [f32; 2],
    pub zoom: Option<ImageCrop>,
}

pub fn current_check(s: &State) -> Option<ConditionCheck> {
    if s.ops.editor.is_some() {
        return None;
    }
    let review = s.ops.review.as_ref()?;
    let (job, sequence, edit, sha) = s.cards.get(s.selected)?.source.key();
    if job != review.request.job_id
        || sequence != review.request.sequence
        || edit != review.source_edit_id
        || sha != review.source_sha256
    {
        return None;
    }
    let mut check: ConditionCheck =
        serde_json::from_value(review.extensions.get("conditionCheck")?.clone()).ok()?;
    check.findings.retain(|f| geometry::valid(f.region));
    check.findings.truncate(64);
    Some(check)
}
pub fn preview(s: &State, t: tok::Theme) -> El {
    let mut frame = el()
        .key("preview")
        .grow(1.)
        .min_h(120.)
        .min_w(0.)
        .w_full()
        .radius(12.)
        .fill(t.card)
        .clip_rounded(12.)
        .on_layout(|s: &mut State, [_, _, w, h]| s.ops.image_review.frame = [w, h]);
    if let Some(image) = &s.image {
        if image.error().is_some() {
            return frame
                .center()
                .child(text("Image could not be opened").color(t.text_secondary));
        }
        let crop = if s.ops.editor.is_none() {
            s.ops.image_review.zoom.unwrap_or_default()
        } else {
            ImageCrop::default()
        };
        if let Some([x, y, w, h]) = image
            .size()
            .and_then(|size| geometry::placement(s.ops.image_review.frame, size, crop))
        {
            let (left, top) = (x + crop.x as f32 * w, y + crop.y as f32 * h);
            let mut viewport = el()
                .absolute(left, top)
                .w(crop.width as f32 * w)
                .h(crop.height as f32 * h)
                .clip_rounded(0.)
                .child(
                    el().absolute(x - left, y - top)
                        .w(w)
                        .h(h)
                        .pixels_fit(image.pixels(), SurfaceFit::Fill),
                );
            if let Some(check) = current_check(s) {
                for (i, finding) in check.findings.iter().enumerate() {
                    let r = finding.region;
                    viewport = viewport.child(
                        el().absolute(x - left + r.x as f32 * w, y - top + r.y as f32 * h)
                            .w(r.width as f32 * w)
                            .h(r.height as f32 * h)
                            .border(2., rune::oklch(0.75, 0.18, 60.))
                            .role("button")
                            .label(format!("Inspect possible mark {}", i + 1))
                            .focusable(true)
                            .on_press(move |s: &mut State| inspect(s, i)),
                    );
                }
            }
            frame = frame.child(viewport);
        }
    } else {
        let message = if s.loading_image {
            "Loading image…"
        } else {
            s.image_error
                .as_deref()
                .unwrap_or("Saved image unavailable")
        };
        frame = frame
            .center()
            .child(text(message).color(t.text_secondary).wrap());
    }
    frame
}
pub fn controls(s: &State, t: tok::Theme) -> El {
    let mut panel = col().gap(6.);
    if s.ops.image_review.zoom.is_some() {
        panel = panel.child(button("Show whole image", "close-mark", true, t, |s| {
            s.ops.image_review.zoom = None
        }));
    }
    if let Some(check) = current_check(s) {
        for (i, finding) in check.findings.iter().enumerate() {
            panel = panel
                .child(button(
                    &format!("Inspect possible mark {}", i + 1),
                    &format!("inspect-mark-{i}"),
                    true,
                    t,
                    move |s| inspect(s, i),
                ))
                .child(
                    text(&finding.explanation)
                        .font(type_::LABEL)
                        .wrap()
                        .color(t.text_secondary),
                );
        }
    }
    panel
}
fn inspect(s: &mut State, index: usize) {
    s.ops.image_review.zoom = current_check(s).and_then(|c| {
        c.findings
            .get(index)
            .and_then(|f| geometry::expanded(f.region))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use scanner_types::{
        device::Device,
        identification::{CardIdentification, IdentificationRequest},
        scan::{ScanRecord, ScanRequest},
    };
    #[test]
    fn overlays_require_the_exact_identified_source_and_ignore_invalid_regions() {
        let mut record =
            ScanRecord::new(Device::default(), ScanRequest::default(), "/fixture".into());
        record.images=vec![serde_json::from_value(serde_json::json!({"sequence":1,"relativePath":"original.png","bytes":100,"sha256":"abc","width":30,"height":40,"imageCount":1,"uniformType":"public.png","side":"back"})).unwrap()];
        let mut identity = CardIdentification::pending(
            IdentificationRequest {
                id: uuid::Uuid::new_v4(),
                job_id: record.id,
                sequence: 1,
                language: Default::default(),
                extensions: Default::default(),
            },
            "abc".into(),
            None,
        );
        identity.extensions.insert("conditionCheck".into(),serde_json::json!({"method":"fixture","state":"checked","limitations":[],"findings":[{"kind":"edgeWhitening","explanation":"Possible mark","region":{"x":0.03,"y":0.4,"width":0.02,"height":0.01}},{"kind":"edgeWhitening","explanation":"Invalid","region":{"x":1.1,"y":0.4,"width":0.02,"height":0.01}}]}));
        record.replace_identification(&identity).unwrap();
        let mut state = State::default();
        state.receive_record(record.clone());
        assert_eq!(current_check(&state).unwrap().findings.len(), 1);
        inspect(&mut state, 0);
        assert!(state.ops.image_review.zoom.is_some());
        identity.source_sha256 = "different".into();
        record.replace_identification(&identity).unwrap();
        state.receive_record(record.clone());
        assert!(current_check(&state).is_none());
        identity.source_sha256 = "abc".into();
        identity.source_edit_id = Some(uuid::Uuid::new_v4());
        record.replace_identification(&identity).unwrap();
        state.receive_record(record);
        assert!(current_check(&state).is_none());
    }
}
