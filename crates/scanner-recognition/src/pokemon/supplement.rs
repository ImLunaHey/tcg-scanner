use crate::{
    matching::footer_orientation_score, ocr::Tesseract, readings::Options, symbols::rarity,
};
use scanner_types::{
    cards::CardLanguage, printed::UncataloguedCard, recognition::RecognitionEvidence,
};
use std::path::Path;

pub(super) fn apply(
    mut evidence: RecognitionEvidence,
    bytes: &[u8],
    language: CardLanguage,
    library: &Path,
    data: &Path,
    options: Options,
) -> RecognitionEvidence {
    if language != CardLanguage::English {
        return evidence;
    }
    let mut printed = evidence
        .candidates
        .is_empty()
        .then(|| super::printed::read(&evidence.lines))
        .flatten();
    if refine(&mut evidence, &mut printed, bytes, library, data, options).is_err() {
        evidence.warnings.push("Printed symbol/set-code refinement was unavailable; existing OCR and candidate evidence are retained.".into());
    }
    if let Some(printed) = printed {
        evidence
            .extensions
            .insert("uncataloguedCard".into(), serde_json::json!(printed));
        evidence.warnings.push("Printed details read locally; no matching catalogue entry. These are OCR transcriptions, not a verified printing. Catalogue lookup will retry automatically.".into());
    }
    evidence
}

fn refine(
    evidence: &mut RecognitionEvidence,
    printed: &mut Option<UncataloguedCard>,
    bytes: &[u8],
    library: &Path,
    data: &Path,
    options: Options,
) -> scanner_types::error::Result<()> {
    let image =
        crate::preprocess::thumbnail(&scanner_images::decode::decode(bytes)?.to_rgb8(), 2400);
    let rotated = match evidence.orientation {
        1 => image::imageops::rotate90(&image),
        2 => image::imageops::rotate180(&image),
        3 => image::imageops::rotate270(&image),
        _ => image,
    };
    let mut ocr = Tesseract::new(library, data, "en")?;
    if let Some(deadline) = options.deadline {
        ocr.set_deadline(deadline);
    }
    if let Some(symbol) = rarity::analyze(&rotated, &ocr.read_footer_words(&rotated)?) {
        rarity::apply(&symbol, &mut evidence.candidates);
        if evidence.candidates.iter().any(|c| {
            c.evidence
                .iter()
                .any(|e| e.starts_with("Rarity symbol conflict:"))
        }) {
            evidence.warnings.push("Printed rarity and catalogue metadata disagree for a candidate. Review the identity; rarity does not establish foil finish.".into());
        }
        evidence
            .extensions
            .insert("raritySymbol".into(), serde_json::json!(symbol));
    }
    if evidence.candidates.is_empty()
        && printed.is_none()
        && footer_orientation_score(&evidence.lines) > 0
    {
        let mut combined = evidence.lines.clone();
        combined.extend(ocr.read_set_code(&rotated, false)?);
        *printed = super::printed::read(&combined);
        if printed.is_some() {
            evidence.lines = combined;
        }
    }
    Ok(())
}
