use super::Geometry;
use scanner_types::binder::CardBox;
pub(super) fn decode(values: &[f32], g: Geometry) -> Vec<CardBox> {
    let n = values.len() / 5;
    let mut candidates: Vec<_> = (0..n)
        .filter_map(|i| {
            let score = values[4 * n + i];
            let (cx, cy, w, h) = (
                values[i],
                values[n + i],
                values[2 * n + i],
                values[3 * n + i],
            );
            if ![score, cx, cy, w, h].iter().all(|v| v.is_finite())
                || !(0.35..=1.).contains(&score)
                || w <= 0.
                || h <= 0.
            {
                return None;
            }
            let x = ((cx - w / 2. - g.left) / g.scale / g.width).clamp(0., 1.);
            let y = ((cy - h / 2. - g.top) / g.scale / g.height).clamp(0., 1.);
            let right = ((cx + w / 2. - g.left) / g.scale / g.width).clamp(0., 1.);
            let bottom = ((cy + h / 2. - g.top) / g.scale / g.height).clamp(0., 1.);
            (right - x >= 0.03 && bottom - y >= 0.03).then_some(CardBox {
                x,
                y,
                width: right - x,
                height: bottom - y,
                confidence: score,
            })
        })
        .collect();
    candidates.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    let mut cards = Vec::new();
    for candidate in candidates {
        if cards.iter().all(|other| overlap(&candidate, other) < 0.45) {
            cards.push(candidate);
        }
        if cards.len() == 24 {
            break;
        }
    }
    cards.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    cards
}
fn overlap(a: &CardBox, b: &CardBox) -> f32 {
    let w = ((a.x + a.width).min(b.x + b.width) - a.x.max(b.x)).max(0.);
    let h = ((a.y + a.height).min(b.y + b.height) - a.y.max(b.y)).max(0.);
    let intersection = w * h;
    intersection / (a.width * a.height + b.width * b.height - intersection)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suppresses_duplicate_boxes_and_maps_padding_back_to_the_photo() {
        let g = Geometry {
            width: 300.,
            height: 600.,
            scale: 640. / 600.,
            left: 160.,
            top: 0.,
        };
        let cards = decode(
            &[320., 321., 320., 321., 300., 300., 500., 500., 0.9, 0.8],
            g,
        );
        assert_eq!(cards.len(), 1);
        assert!((cards[0].x - 0.03125).abs() < 0.001);
        assert!((cards[0].height - 0.78125).abs() < 0.001);
        assert!(decode(&[f32::NAN; 5], g).is_empty());
        assert!(decode(&[320., 320., -5., 200., 0.9], g).is_empty());
    }
}
