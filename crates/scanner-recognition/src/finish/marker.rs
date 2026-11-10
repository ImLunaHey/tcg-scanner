use super::{background::Background, border, decision};
use crate::ocr::Tesseract;
use image::{RgbImage, imageops};
use scanner_types::{
    error::{Result, ScannerError},
    finish::{FinishEstimate, FinishState, FinishUncertainty as Reason},
    images::{CardPreparation, ImageCrop},
    recognition::{CardCandidate, CardFinish, RecognitionEvidence, RecognizedLine},
};
use std::sync::LazyLock;

static MODERN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^(sv|me)[0-9]{2}(\.[0-9][a-z]?)?$").unwrap());
static CODE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[A-Z0-9]{3}EN$").unwrap());
pub(super) fn modern_set(id: &str) -> bool {
    MODERN.is_match(id)
}
const LIMITATIONS: [&str; 2] = [
    "Experimental estimate: inspect the physical card before confirming finish.",
    "Uses printed set-code contrast, catalogue variants and eligible prepared-card border texture, not measured reflectivity. Special/promotional printings may differ.",
];
pub(super) fn analyze(
    bytes: &[u8],
    evidence: &RecognitionEvidence,
    preparation: Option<&CardPreparation>,
    mut ocr: Option<&mut Tesseract>,
) -> Result<FinishEstimate> {
    let card = evidence.candidates.first();
    if evidence.candidates.len() != 1
        || !card.is_some_and(|c| {
            c.evidence
                .iter()
                .any(|s| s.starts_with("Collector number "))
        })
    {
        return Ok(result(
            None,
            FinishState::Uncertain,
            "A single printing with a readable collector number is required.",
            Some(Reason::AmbiguousIdentity),
        ));
    }
    let card = card.unwrap();
    if let Some(result) = unsupported_or_catalogue(card) {
        return Ok(result);
    }
    let pixels = upright(bytes, evidence.orientation)?;
    let mut markers = locate(&evidence.lines);
    if markers.is_empty()
        && let Some(ocr) = ocr.as_deref_mut()
    {
        markers = locate(&ocr.read_set_code(&pixels, false)?);
    }
    let mut enlarged = false;
    if markers.is_empty()
        && let Some(ocr) = ocr.as_deref_mut()
    {
        markers = locate(&ocr.read_set_code(&pixels, true)?);
        enlarged = markers.len() == 1;
    }
    if markers.len() != 1 {
        return Ok(result(
            Some(card),
            FinishState::Uncertain,
            "The English set-code box could not be located unambiguously.",
            Some(Reason::MarkerNotFound),
        ));
    }
    let marker = &markers[0];
    let mut measured = measure(&pixels, marker, card, evidence.orientation)?;
    if enlarged {
        measured
            .evidence
            .push("Set-code box located by re-reading enlarged pixels locally.".into());
    }
    if measured.uncertainty_reason == Some(Reason::MarkerContrast)
        && !enlarged
        && let Some(ocr) = ocr
    {
        let refined = locate(&ocr.read_set_code(&pixels, true)?);
        if refined.len() == 1 && tighter(&refined[0], marker) {
            let mut retry = measure(&pixels, &refined[0], card, evidence.orientation)?;
            if retry.marker_polarity.is_some() {
                retry.evidence.push("An enlarged local OCR pass isolated the same set code in a tighter box, excluding neighbouring print.".into());
                measured = retry;
            }
        }
    }
    if preparation
        .is_some_and(|p| p.orientation_verified && p.orientation_basis.as_deref() == Some("text"))
        && card.category.as_deref() == Some("Pokemon")
        && measured.marker_polarity.as_deref() == Some("dark")
        && (measured.state == FinishState::Estimated
            || measured.uncertainty_reason == Some(Reason::MultipleRegularFinishes))
    {
        measured = decision::combine(measured, card, border::analyze(&pixels));
    }
    Ok(measured)
}
fn unsupported_or_catalogue(card: &CardCandidate) -> Option<FinishEstimate> {
    if let Some(result) = catalogue_ex(card) {
        return Some(result);
    }
    if card.set_id == "sve" {
        return Some(result(
            Some(card),
            FinishState::Unsupported,
            "Basic Energy foil variants need validation. A dark SVE set-code box does not yet establish non-holo.",
            None,
        ));
    }
    let numbered = card
        .local_id
        .parse::<u32>()
        .ok()
        .zip(card.official_set_count)
        .is_some_and(|(n, total)| n <= total);
    if !MODERN.is_match(&card.set_id)
        || !numbered
        || !card.available_finishes.contains(&CardFinish::ReverseHolo)
    {
        return Some(result(
            Some(card),
            FinishState::Unsupported,
            "This printing is outside the supported modern set-code layout.",
            None,
        ));
    }
    None
}

fn result(
    card: Option<&CardCandidate>,
    state: FinishState,
    message: &str,
    reason: Option<Reason>,
) -> FinishEstimate {
    let mut result = FinishEstimate::new(
        state,
        card.map(|c| c.id.clone()),
        vec![message.into()],
        reason,
    );
    result.limitations = LIMITATIONS.map(str::to_owned).to_vec();
    result
}
fn catalogue_ex(card: &CardCandidate) -> Option<FinishEstimate> {
    if !card.set_id.starts_with("sv")
        || !MODERN.is_match(&card.set_id)
        || card.id != format!("{}-{}", card.set_id, card.local_id)
        || !card.local_id.bytes().all(|b| b.is_ascii_digit())
        || !card.local_id.parse::<u32>().is_ok_and(|n| n > 0)
        || !card.official_set_count.is_some_and(|n| n > 0)
        || !card
            .evidence
            .contains(&format!("Collector number {}", card.local_id))
        || !card.name.ends_with(" ex")
        || card.available_finishes.is_empty()
        || card
            .available_finishes
            .iter()
            .any(|f| *f != CardFinish::Holo)
    {
        return None;
    }
    let mut result = result(
        Some(card),
        FinishState::Estimated,
        "The exact Scarlet & Violet Pokémon ex printing lists only holo in the catalogue.",
        None,
    );
    result.suggested_finish = Some(CardFinish::Holo);
    result.method = "modern-ex-catalogue-v1".into();
    result.limitations=vec!["Catalogue-based inference, not measured reflectivity. Incorrect identification, missing catalogue variants or special printings can invalidate it.".into(),"Inspect the physical card before confirming finish; this does not confirm authenticity or a particular foil texture.".into()];
    Some(result)
}
fn code(line: &RecognizedLine) -> String {
    line.text
        .to_uppercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}
fn locate(lines: &[RecognizedLine]) -> Vec<RecognizedLine> {
    lines
        .iter()
        .filter(|line| {
            line.confidence >= 0.5
                && (0.04..=0.18).contains(&line.x)
                && (0.015..0.08).contains(&line.y)
                && (0.03..=0.13).contains(&line.width)
                && (0.008..=0.035).contains(&line.height)
                && CODE.is_match(&code(line))
        })
        .cloned()
        .collect()
}
fn tighter(refined: &RecognizedLine, original: &RecognizedLine) -> bool {
    let area = refined.width * refined.height / (original.width * original.height);
    code(refined) == code(original)
        && (0.3..=0.8).contains(&area)
        && refined.x >= original.x - 0.004
        && refined.y >= original.y - 0.004
        && refined.x + refined.width <= original.x + original.width + 0.004
        && refined.y + refined.height <= original.y + original.height + 0.004
}
pub(super) fn upright(bytes: &[u8], turns: u32) -> Result<RgbImage> {
    if turns > 3 {
        return Err(ScannerError::new(
            "invalidParams",
            "Invalid finish orientation.",
        ));
    }
    let decoded = scanner_images::decode::decode(bytes)?;
    if u64::from(decoded.width()) * u64::from(decoded.height()) > 40_000_000 {
        return Err(ScannerError::new(
            "resourceLimit",
            "Finish analysis requires at most 40 megapixels.",
        ));
    }
    let pixels = decoded.to_rgb8();
    Ok(match turns {
        1 => imageops::rotate90(&pixels),
        2 => imageops::rotate180(&pixels),
        3 => imageops::rotate270(&pixels),
        _ => pixels,
    })
}
fn measure(
    pixels: &RgbImage,
    marker: &RecognizedLine,
    card: &CardCandidate,
    turns: u32,
) -> Result<FinishEstimate> {
    let region = ImageCrop {
        x: marker.x - 0.002,
        y: 1. - marker.y - marker.height - 0.0015,
        width: marker.width + 0.004,
        height: marker.height + 0.003,
    };
    let (w, h) = (f64::from(pixels.width()), f64::from(pixels.height()));
    let left = (region.x * w).floor().max(0.) as u32;
    let top = (region.y * h).floor().max(0.) as u32;
    let right = ((region.x + region.width) * w).ceil().min(w) as u32;
    let bottom = ((region.y + region.height) * h).ceil().min(h) as u32;
    let (width, height) = (right.saturating_sub(left), bottom.saturating_sub(top));
    if width < 12 || height < 6 || u64::from(width) * u64::from(height) > 100_000 {
        return Err(ScannerError::new(
            "image",
            "Set-code box is too small or exceeds analysis limits.",
        ));
    }
    let mut values = Vec::with_capacity((width * height) as usize);
    for y in top..bottom {
        for x in left..right {
            values.push(border::luminance(pixels.get_pixel(x, y).0));
        }
    }
    let dark = values.iter().filter(|v| **v < 0.30).count() as f64 / values.len() as f64;
    let light = values.iter().filter(|v| **v > 0.70).count() as f64 / values.len() as f64;
    let background = Background::measure(&values, width as usize, height as usize);
    values.sort_by(f64::total_cmp);
    let median = values[values.len() / 2];
    let mut result = decision::contrast(card, median, dark, light, &background);
    result.limitations = LIMITATIONS.map(str::to_owned).to_vec();
    result.region = Some(region);
    result.source_quarter_turns = Some(turns);
    result.median_luminance = Some(median);
    result.dark_background_area_fraction = Some(background.dark);
    result.light_background_area_fraction = Some(background.light);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn card() -> CardCandidate {
        serde_json::from_value(serde_json::json!({"id":"sv09-177","name":"Salamence ex","setID":"sv09","setName":"Journey Together","localID":"177","officialSetCount":159,"evidence":["Collector number 177"],"availableFinishes":["holo"]})).unwrap()
    }
    #[test]
    fn catalogue_ex_requires_exact_numbered_modern_printing_and_only_holo() {
        let mut card = card();
        assert_eq!(
            catalogue_ex(&card).unwrap().suggested_finish,
            Some(CardFinish::Holo)
        );
        card.available_finishes.push(CardFinish::Normal);
        assert!(catalogue_ex(&card).is_none());
        card.available_finishes.pop();
        card.id = "another-printing".into();
        assert!(catalogue_ex(&card).is_none());
        card = self::card();
        card.evidence.clear();
        assert!(catalogue_ex(&card).is_none());
        card = self::card();
        card.set_id = "me01".into();
        card.id = "me01-177".into();
        assert!(catalogue_ex(&card).is_none());
    }
    #[test]
    fn marker_refinement_requires_same_text_and_a_contained_smaller_box() {
        let original = RecognizedLine {
            text: "SVI EN".into(),
            confidence: 0.9,
            x: 0.06,
            y: 0.03,
            width: 0.1,
            height: 0.02,
        };
        assert_eq!(locate(std::slice::from_ref(&original)).len(), 1);
        let mut refined = original.clone();
        refined.width = 0.06;
        assert!(tighter(&refined, &original));
        refined.text = "SFA EN".into();
        assert!(!tighter(&refined, &original));
        refined.text = original.text.clone();
        refined.x = 0.16;
        assert!(!tighter(&refined, &original));
        refined.x = f64::NAN;
        assert!(locate(&[refined]).is_empty());
    }
    #[test]
    fn korean_and_basic_energy_never_inherit_english_marker_rules() {
        let mut card = card();
        let evidence = RecognitionEvidence {
            candidates: vec![card.clone()],
            ..Default::default()
        };
        let result = super::super::ordinary(
            &[],
            &evidence,
            None,
            scanner_types::cards::CardLanguage::Korean,
            None,
        );
        assert_eq!(result.uncertainty_reason, Some(Reason::LanguageUnsupported));
        card.set_id = "sve".into();
        assert_eq!(
            unsupported_or_catalogue(&card).unwrap().state,
            FinishState::Unsupported
        );
    }
}
