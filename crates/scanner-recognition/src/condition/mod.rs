//! Candidate marks on prepared standard blue backs. Never assigns a card grade.
mod border;
mod components;
use scanner_types::{
    condition::{ConditionCheck, ConditionState},
    images::CardPreparation,
};

const LIMITATIONS: [&str; 4] = [
    "Experimental candidate marks, not confirmed damage or a condition grade.",
    "Light areas connected to the image boundary are excluded as possible backing; wear merging into a white background cannot be reliably assessed.",
    "Checks only the blue outer border of a prepared standard back. Fronts, artwork, the outermost rim and hidden surfaces are not assessed.",
    "Dust, print variation and scan artifacts can resemble wear. Fine scratches, dents and creases may be invisible; no flagged marks does not mean Near Mint.",
];

pub fn check(bytes: &[u8], preparation: Option<&CardPreparation>) -> ConditionCheck {
    let mut result = analyze(bytes, preparation);
    if let Some(preparation) = preparation {
        result.limitations.extend(
            preparation
                .warnings
                .iter()
                .filter(|s| s.contains("scan boundary"))
                .cloned(),
        );
    }
    result.limitations.extend(LIMITATIONS.map(str::to_owned));
    result
}
fn analyze(bytes: &[u8], preparation: Option<&CardPreparation>) -> ConditionCheck {
    let unavailable = |state, reason: &str| ConditionCheck::new(state, vec![reason.into()]);
    let Some(preparation) = preparation else {
        return unavailable(
            ConditionState::Unsupported,
            "Not assessed: this image has no automatic card preparation.",
        );
    };
    if preparation.orientation_basis.as_deref() == Some("text") {
        return unavailable(
            ConditionState::Unsupported,
            "Not assessed: this front image is outside the blue-back border check's scope.",
        );
    }
    let Ok(decoded) = scanner_images::decode::decode(bytes) else {
        return unavailable(
            ConditionState::Inconclusive,
            "The saved image could not be analyzed. Inspect the physical card; no grade was assigned.",
        );
    };
    let (w, h) = (decoded.width(), decoded.height());
    if w.min(h) < 500
        || w.max(h) < 700
        || u64::from(w) * u64::from(h) > 40_000_000
        || !(0.65..=0.80).contains(&(f64::from(w.min(h)) / f64::from(w.max(h))))
    {
        return unavailable(
            ConditionState::Inconclusive,
            "Not assessed: needs one card image of at least 500 × 700 pixels in either orientation, up to 32 MiB and 40 megapixels.",
        );
    }
    let mut rgba = decoded.to_rgba8();
    // Match the existing premultiplied sRGB analysis, including transparent corners.
    for pixel in rgba.pixels_mut() {
        let alpha = u16::from(pixel[3]);
        for c in 0..3 {
            pixel[c] = (u16::from(pixel[c]) * alpha / 255) as u8;
        }
    }
    let pixels = image::DynamicImage::ImageRgba8(rgba).to_rgb8();
    let prepared = preparation.orientation_verified
        && preparation.orientation_basis.as_deref() == Some("pokemon-back-pattern");
    if !prepared && crate::orientation::pokemon_back_layout(&pixels).is_none() {
        return unavailable(
            ConditionState::Unsupported,
            "Not assessed: the standard blue-back colour and layout pattern could not be confirmed.",
        );
    }
    let pixels = crate::preprocess::thumbnail(&pixels, 1200);
    let mut result = border::inspect(&pixels);
    result.back_recognition = Some(
        if prepared {
            "preparation"
        } else {
            "colour-layout"
        }
        .into(),
    );
    if result.state == ConditionState::Checked {
        result.coverage = Some(
            [
                "backBorderWhitening",
                "backCornerWhitening",
                "backBorderLinearMarks",
            ]
            .map(str::to_owned)
            .to_vec(),
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use scanner_types::{Extensions, images::ImageCrop};
    #[test]
    fn unsupported_fronts_and_invalid_sources_never_receive_a_grade() {
        let mut prep = CardPreparation {
            crop: ImageCrop::default(),
            quarter_turns: 0,
            orientation_verified: true,
            warnings: vec![],
            source_crop: None,
            deskew_degrees: None,
            orientation_basis: Some("text".into()),
            method: "fixture".into(),
            extensions: Extensions::new(),
        };
        assert_eq!(check(&[], None).state, ConditionState::Unsupported);
        assert_eq!(check(&[], Some(&prep)).state, ConditionState::Unsupported);
        prep.orientation_basis = Some("pokemon-back-pattern".into());
        let result = check(&[], Some(&prep));
        assert_eq!(result.state, ConditionState::Inconclusive);
        assert!(
            result
                .limitations
                .iter()
                .any(|s| s.contains("does not mean Near Mint"))
        );
    }
}
