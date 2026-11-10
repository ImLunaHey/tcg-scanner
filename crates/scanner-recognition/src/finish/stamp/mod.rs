//! Two-resolution local mark checks. Shape scores are not probabilities.
mod anniversary;
mod components;
mod glyph;
mod pumpkin;
mod templates;
use image::{RgbImage, imageops};
use scanner_types::{
    edition::{StampEvidence, StampState},
    error::{Result, ScannerError},
    images::ImageCrop,
};

struct Match {
    score: f64,
    region: ImageCrop,
    dark_face: bool,
}
/// Requires an independently prepared, upright card front. Orientation is
/// clockwise quarter turns from the exact supplied image.
pub fn analyze(bytes: &[u8], orientation: u32) -> Result<StampEvidence> {
    let pixels = decode(bytes, orientation)?;
    let full = crate::preprocess::thumbnail(&pixels, 1260);
    if full.height() < 600 || !(0.65..=0.80).contains(&(full.width() as f64 / full.height() as f64))
    {
        return Ok(evidence(
            StampState::Unsupported,
            orientation,
            false,
            None,
            None,
        ));
    }
    let a = anniversary::find(&full);
    let small = crate::preprocess::thumbnail(&pixels, 1050);
    let b = anniversary::find(&small);
    let same = a.as_ref().zip(b.as_ref()).is_some_and(|(a, b)| {
        (a.region.x - b.region.x).abs() < 0.025 && (a.region.y - b.region.y).abs() < 0.025
    });
    let score = a
        .as_ref()
        .map_or(0., |m| m.score)
        .min(b.as_ref().map_or(0., |m| m.score));
    let state = if same && score >= 0.79 {
        StampState::Present
    } else if a.as_ref().is_some_and(|m| m.score >= 0.75) {
        StampState::Uncertain
    } else {
        StampState::Absent
    };
    if state != StampState::Absent {
        return Ok(evidence(state, orientation, true, a, b));
    }
    Ok(pumpkin_evidence(
        &full,
        &crate::preprocess::thumbnail(&pixels, 630),
        orientation,
    ))
}
/// Compare the same pumpkin mark on public reference artwork, independently of
/// the anniversary priority used on scanned fronts.
pub fn pumpkin(bytes: &[u8], orientation: u32) -> Result<StampEvidence> {
    let pixels = decode(bytes, orientation)?;
    let full = crate::preprocess::thumbnail(&pixels, 1260);
    if full.height() < 600 || !(0.65..=0.80).contains(&(full.width() as f64 / full.height() as f64))
    {
        return Ok(evidence(
            StampState::Unsupported,
            orientation,
            false,
            None,
            None,
        ));
    }
    Ok(pumpkin_evidence(
        &full,
        &crate::preprocess::thumbnail(&pixels, 630),
        orientation,
    ))
}
fn decode(bytes: &[u8], orientation: u32) -> Result<RgbImage> {
    if orientation > 3 {
        return Err(ScannerError::new(
            "invalidParams",
            "Stamp analysis requires a valid quarter-turn orientation.",
        ));
    }
    let decoded = scanner_images::decode::decode(bytes)?;
    if u64::from(decoded.width()) * u64::from(decoded.height()) > 40_000_000 {
        return Err(ScannerError::new(
            "resourceLimit",
            "Stamp analysis requires at most 40 megapixels.",
        ));
    }
    let mut rgba = decoded.to_rgba8();
    for p in rgba.pixels_mut() {
        for c in 0..3 {
            p[c] = (u16::from(p[c]) * u16::from(p[3]) / 255) as u8;
        }
    }
    let pixels = image::DynamicImage::ImageRgba8(rgba).to_rgb8();
    Ok(match orientation {
        1 => imageops::rotate90(&pixels),
        2 => imageops::rotate180(&pixels),
        3 => imageops::rotate270(&pixels),
        _ => pixels,
    })
}
fn pumpkin_evidence(full: &RgbImage, small: &RgbImage, orientation: u32) -> StampEvidence {
    let (a, b) = (pumpkin::find(full), pumpkin::find(small));
    let (score_a, score_b) = (
        a.as_ref().map_or(0., |m| m.score),
        b.as_ref().map_or(0., |m| m.score),
    );
    let agrees = a
        .as_ref()
        .zip(b.as_ref())
        .is_some_and(|(a, b)| a.dark_face && b.dark_face && overlap(&a.region, &b.region) >= 0.8);
    let state = if score_a >= 0.88 && score_b >= 0.88 && agrees {
        StampState::Present
    } else if score_a.max(score_b) >= 0.75 {
        StampState::Uncertain
    } else {
        StampState::Absent
    };
    evidence(state, orientation, false, a, b)
}
fn overlap(a: &ImageCrop, b: &ImageCrop) -> f64 {
    let intersection = (a.x + a.width).min(b.x + b.width) - a.x.max(b.x);
    let intersection =
        intersection.max(0.) * ((a.y + a.height).min(b.y + b.height) - a.y.max(b.y)).max(0.);
    let union = a.width * a.height + b.width * b.height - intersection;
    if union > 0. { intersection / union } else { 0. }
}
fn evidence(
    state: StampState,
    orientation: u32,
    anniversary: bool,
    a: Option<Match>,
    b: Option<Match>,
) -> StampEvidence {
    let region = if state == StampState::Absent {
        None
    } else {
        a.as_ref().or(b.as_ref()).map(|m| m.region)
    };
    let (first, second) = (a.as_ref().map(|m| m.score), b.as_ref().map(|m| m.score));
    let message = match (state, anniversary) {
        (StampState::Present, true) => {
            "Separated 3 and 0 cheek shapes agree at two resolutions in the anniversary-mark region."
        }
        (StampState::Present, false) => {
            "Orange pumpkin silhouette, paired ears and dark facial cutouts agree at two resolutions."
        }
        (StampState::Uncertain, _) => {
            "A mark resembles the expected shape, but its details do not agree strongly enough at both resolutions."
        }
        (StampState::Unsupported, _) => {
            "Stamp analysis requires a prepared portrait card at a readable resolution."
        }
        _ => "No matching pumpkin mark was found. This does not rule out other stamp designs.",
    };
    StampEvidence { state,region,source_quarter_turns:orientation,
        kind:if anniversary { "pikachu-30th-anniversary" } else { "pikachu-pumpkin" }.into(),
        method:if anniversary { "anniversary-cheeks-rust-v1" } else { "pumpkin-shape-rust-v1" }.into(),
        similarity:Some(first.unwrap_or(0.).min(second.unwrap_or(0.))),
        baseline_similarity:first,downsampled_similarity:second,
        evidence:vec![message.into(),"Shape similarity is not a probability, edition year, authenticity check or finish measurement.".into()] }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn encoded(pixels: &RgbImage) -> Vec<u8> {
        let mut out = std::io::Cursor::new(vec![]);
        image::DynamicImage::ImageRgb8(pixels.clone())
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }
    fn pumpkin_image(face: image::Rgb<u8>) -> RgbImage {
        let mut image = RgbImage::from_pixel(900, 1260, image::Rgb([240; 3]));
        for y in 0..160 {
            for x in 0..192 {
                let ink = templates::PUMPKIN[y * 32 / 160][x * 32 / 192] == b'#';
                image.put_pixel(
                    650 + x as u32,
                    450 + y as u32,
                    if ink {
                        image::Rgb([240, 125, 20])
                    } else {
                        face
                    },
                );
            }
        }
        image
    }
    #[test]
    fn pumpkin_requires_dark_face_and_consistent_rotated_location() {
        let image = pumpkin_image(image::Rgb([10; 3]));
        let reference = pumpkin(&encoded(&image), 0).unwrap();
        assert_eq!(reference.state, StampState::Present);
        for (pixels, turns) in [
            (imageops::rotate270(&image), 1),
            (imageops::rotate180(&image), 2),
            (imageops::rotate90(&image), 3),
        ] {
            let value = pumpkin(&encoded(&pixels), turns).unwrap();
            assert_eq!(value.state, StampState::Present);
            assert!(
                overlap(
                    value.region.as_ref().unwrap(),
                    reference.region.as_ref().unwrap()
                ) > 0.99
            );
        }
        let hollow = pumpkin_image(image::Rgb([240; 3]));
        assert_eq!(
            pumpkin(&encoded(&hollow), 0).unwrap().state,
            StampState::Uncertain
        );
        assert_eq!(
            pumpkin(&encoded(&RgbImage::new(900, 1260)), 0)
                .unwrap()
                .state,
            StampState::Absent
        );
    }
    #[test]
    fn anniversary_requires_separate_correct_digits_and_takes_priority() {
        let mut image = RgbImage::from_pixel(900, 1260, image::Rgb([240; 3]));
        for (left, mask) in [(660, templates::THREE), (750, templates::ZERO)] {
            for (y, row) in mask.iter().enumerate() {
                for (x, &ink) in row.iter().enumerate() {
                    if ink == b'#' {
                        image.put_pixel(left + x as u32, 600 + y as u32, image::Rgb([200, 30, 30]));
                    }
                }
            }
        }
        let found = analyze(&encoded(&image), 0).unwrap();
        assert_eq!(found.kind, "pikachu-30th-anniversary");
        assert_eq!(found.state, StampState::Present);
        assert!(analyze(b"invalid", 0).is_err());
        assert!(analyze(&encoded(&image), 4).is_err());
        assert_eq!(
            analyze(&encoded(&RgbImage::new(400, 560)), 0)
                .unwrap()
                .state,
            StampState::Unsupported
        );
    }
}
