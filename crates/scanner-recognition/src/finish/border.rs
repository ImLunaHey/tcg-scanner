use image::RgbImage;
use scanner_types::finish::{BorderFinishEvidence, BorderState};
pub(super) const THRESHOLD: f64 = 0.008213026997467985;

pub(super) fn analyze(upright: &RgbImage) -> BorderFinishEvidence {
    let full = crate::preprocess::thumbnail(upright, 1260);
    let aspect = f64::from(full.width()) / f64::from(full.height());
    if full.height() < 600 || !(0.65..=0.80).contains(&aspect) {
        return unavailable(BorderState::Unsupported);
    }
    let smaller = crate::preprocess::thumbnail(upright, 630);
    consensus(
        minimum_band(&full, false),
        minimum_band(&full, true),
        minimum_band(&smaller, false),
    )
}
pub(super) fn unavailable(state: BorderState) -> BorderFinishEvidence {
    BorderFinishEvidence {
        state,
        threshold: THRESHOLD,
        baseline: None,
        inset: None,
        downsampled: None,
        method: "independent-border-bands-rust-v1".into(),
    }
}
fn consensus(baseline: f64, inset: f64, downsampled: f64) -> BorderFinishEvidence {
    let values = [baseline, inset, downsampled];
    if values.iter().any(|v| !v.is_finite() || *v < 0.) {
        return unavailable(BorderState::Unsupported);
    }
    let state = match values.iter().filter(|v| **v > THRESHOLD).count() {
        3 => BorderState::Present,
        0 => BorderState::Absent,
        _ => BorderState::Unstable,
    };
    BorderFinishEvidence {
        baseline: Some(baseline),
        inset: Some(inset),
        downsampled: Some(downsampled),
        ..unavailable(state)
    }
}
fn minimum_band(pixels: &RgbImage, inset: bool) -> f64 {
    let (x0, x1) = if inset { (0.02, 0.035) } else { (0.015, 0.04) };
    band(pixels, x0, x1).min(band(pixels, 1. - x1, 1. - x0))
}
fn band(pixels: &RgbImage, x0: f64, x1: f64) -> f64 {
    let (w, h) = pixels.dimensions();
    let start = (x0 * f64::from(w)) as u32;
    let end = (x1 * f64::from(w)) as u32;
    let mut rows = vec![];
    for y in (0.12 * f64::from(h)) as u32..(0.86 * f64::from(h)) as u32 {
        let sum: f64 = (start..end)
            .map(|x| luminance(pixels.get_pixel(x, y).0))
            .sum();
        rows.push(sum / f64::from((end - start).max(1)));
    }
    let radius = ((5. * f64::from(h) / 1260.).round() as usize).max(1);
    let mut energy = 0.;
    for (i, &value) in rows.iter().enumerate() {
        let lo = i.saturating_sub(radius);
        let hi = (rows.len() - 1).min(i + radius);
        let residual = value - rows[lo..=hi].iter().sum::<f64>() / (hi - lo + 1) as f64;
        energy += residual * residual;
    }
    (energy / rows.len().max(1) as f64).sqrt()
}
pub(super) fn luminance([r, g, b]: [u8; 3]) -> f64 {
    (0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b)) / 255.
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn consensus_requires_every_independent_measurement_and_finite_values() {
        assert_eq!(
            consensus(THRESHOLD, THRESHOLD, THRESHOLD).state,
            BorderState::Absent
        );
        assert_eq!(consensus(0.02, 0.02, 0.02).state, BorderState::Present);
        assert_eq!(consensus(0.02, 0., 0.02).state, BorderState::Unstable);
        assert_eq!(consensus(f64::NAN, 0., 0.).state, BorderState::Unsupported);
        let plain = RgbImage::from_pixel(700, 1000, image::Rgb([128; 3]));
        assert_eq!(analyze(&plain).state, BorderState::Absent);
        let mut one_border = plain.clone();
        for y in 0..1000 {
            for x in 10..30 {
                one_border.put_pixel(x, y, image::Rgb([if y % 8 < 4 { 255 } else { 0 }; 3]));
            }
        }
        assert_eq!(analyze(&one_border).state, BorderState::Absent);
    }
}
