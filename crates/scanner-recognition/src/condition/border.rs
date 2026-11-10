use super::components::{self, Component};
use image::RgbImage;
use scanner_types::{
    condition::{ConditionCheck, ConditionFinding, ConditionState, FindingKind},
    images::ImageCrop,
};

pub(super) fn inspect(image: &RgbImage) -> ConditionCheck {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let exterior = components::exterior(image);
    let mut mask = vec![false; w * h];
    let (mut border, mut blue) = (0, 0);
    let (inset_x, inset_y, band_x, band_y) = if w > h {
        (0.006, 0.008, 0.032, 0.045)
    } else {
        (0.008, 0.006, 0.045, 0.032)
    };
    for (x, y, pixel) in image.enumerate_pixels() {
        let (nx, ny) = (f64::from(x) / w as f64, f64::from(y) / h as f64);
        let p = y as usize * w + x as usize;
        if nx < inset_x
            || nx > 1. - inset_x
            || ny < inset_y
            || ny > 1. - inset_y
            || !(nx < band_x || nx > 1. - band_x || ny < band_y || ny > 1. - band_y)
            || exterior[p]
        {
            continue;
        }
        border += 1;
        let [r, g, b] = pixel.0.map(f64::from);
        if b > 35. && b > r * 1.3 && b > g * 1.15 {
            blue += 1;
        }
        mask[p] = components::light(pixel);
    }
    if f64::from(blue) / f64::from(border.max(1)) <= 0.55 {
        return ConditionCheck::new(ConditionState::Inconclusive,
            vec!["The border is not consistently blue. Check cropping, lighting and card-back design manually.".into()]);
    }
    let mut found = vec![];
    for start in 0..mask.len() {
        if !mask[start] {
            continue;
        }
        let component = components::collect(&mut mask, start, w, h);
        if let Some(finding) = finding(&component, w, h) {
            found.push((component.area, finding));
        }
    }
    found.sort_by(|(area_a, a), (area_b, b)| {
        area_b
            .cmp(area_a)
            .then(a.region.y.total_cmp(&b.region.y))
            .then(a.region.x.total_cmp(&b.region.x))
    });
    let limits = if found.len() > 24 {
        vec!["Showing the 24 largest candidate marks; inspect the entire card.".into()]
    } else {
        vec![]
    };
    let mut result = ConditionCheck::new(ConditionState::Checked, limits);
    result.analyzed_width = Some(w as u32);
    result.analyzed_height = Some(h as u32);
    result.findings = found
        .into_iter()
        .take(24)
        .map(|(_, finding)| finding)
        .collect();
    result
}
fn finding(c: &Component, w: usize, h: usize) -> Option<ConditionFinding> {
    let (bw, bh) = (c.right - c.left + 1, c.bottom - c.top + 1);
    if c.area < 8.max(w * h / 90_000) || bw < 2 || bh < 2 {
        return None;
    }
    let (cx, cy) = (
        (c.left + c.right) as f64 / (2 * w) as f64,
        (c.top + c.bottom) as f64 / (2 * h) as f64,
    );
    let (corner_x, corner_y) = if w > h { (0.10, 0.12) } else { (0.12, 0.10) };
    let corner = (cx < corner_x || cx > 1. - corner_x) && (cy < corner_y || cy > 1. - corner_y);
    let linear = bw.max(bh) >= 24.max(w.min(h) / 25) && bw.max(bh) >= bw.min(bh) * 6;
    let (kind, explanation) = if linear {
        (
            FindingKind::LinearMark,
            "Elongated light mark in the blue border. Inspect for a scratch or crease; a scan artifact is also possible.",
        )
    } else if corner {
        (
            FindingKind::CornerWhitening,
            "Light patch near a corner. Inspect for whitening, chipped ink or a scan artifact.",
        )
    } else {
        (
            FindingKind::EdgeWhitening,
            "Light patch in the blue edge border. Inspect for whitening, chipped ink or a scan artifact.",
        )
    };
    let (x, y) = (c.left.saturating_sub(3), c.top.saturating_sub(3));
    Some(ConditionFinding {
        kind,
        explanation: explanation.into(),
        region: ImageCrop {
            x: x as f64 / w as f64,
            y: y as f64 / h as f64,
            width: ((c.right + 4).min(w) - x) as f64 / w as f64,
            height: ((c.bottom + 4).min(h) - y) as f64 / h as f64,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn paint(image: &mut RgbImage, x: u32, y: u32, w: u32, h: u32) {
        for yy in y..y + h {
            for xx in x..x + w {
                image.put_pixel(xx, yy, image::Rgb([225; 3]));
            }
        }
    }
    #[test]
    fn backing_artwork_paper_rim_and_noise_are_excluded_but_internal_marks_remain() {
        let mut pixels = RgbImage::from_pixel(700, 1000, image::Rgb([25, 45, 120]));
        paint(&mut pixels, 0, 0, 10, 1000);
        paint(&mut pixels, 690, 0, 10, 1000);
        paint(&mut pixels, 0, 0, 20, 18);
        paint(&mut pixels, 680, 0, 20, 18);
        paint(&mut pixels, 80, 100, 500, 800);
        paint(&mut pixels, 10, 400, 1, 1);
        assert_eq!(inspect(&pixels).state, ConditionState::Checked);
        assert!(inspect(&pixels).findings.is_empty());
        paint(&mut pixels, 15, 60, 7, 7);
        paint(&mut pixels, 680, 500, 7, 7);
        paint(&mut pixels, 280, 14, 45, 3);
        let result = inspect(&pixels);
        assert_eq!(result.findings.len(), 3);
        for kind in [
            FindingKind::CornerWhitening,
            FindingKind::EdgeWhitening,
            FindingKind::LinearMark,
        ] {
            assert!(result.findings.iter().any(|f| f.kind == kind));
        }
        for f in result.findings {
            assert!(f.region.x + f.region.width <= 1. && f.region.y + f.region.height <= 1.);
        }
    }
    #[test]
    fn coordinates_stay_in_the_source_frame_at_all_quarter_turns() {
        let mut pixels = RgbImage::from_pixel(700, 1000, image::Rgb([25, 45, 120]));
        paint(&mut pixels, 12, 60, 7, 7);
        for turn in 0..4 {
            let result = inspect(&pixels);
            assert_eq!(result.state, ConditionState::Checked);
            assert_eq!(result.findings.len(), 1);
            let finding = &result.findings[0];
            assert_eq!(finding.kind, FindingKind::CornerWhitening);
            let (x, y) = (
                finding.region.x + finding.region.width / 2.,
                finding.region.y + finding.region.height / 2.,
            );
            assert_eq!(x > 0.9, matches!(turn, 1 | 2));
            assert_eq!(y > 0.9, matches!(turn, 2 | 3));
            pixels = image::imageops::rotate90(&pixels);
        }
    }
}
