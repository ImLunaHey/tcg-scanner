use crate::{matching::footer_orientation_score, ocr::Tesseract, readings::Options};
use scanner_types::{cards::CardLanguage, recognition::RecognitionEvidence};
use std::path::Path;

pub(super) fn apply(
    mut evidence: RecognitionEvidence,
    bytes: &[u8],
    language: CardLanguage,
    library: &Path,
    data: &Path,
    options: Options,
) -> RecognitionEvidence {
    if language != CardLanguage::English || !evidence.candidates.is_empty() {
        return evidence;
    }
    let mut printed = super::printed::read(&evidence.lines);
    if printed.is_none() && footer_orientation_score(&evidence.lines) > 0 {
        match code_lines(bytes, evidence.orientation, library, data, options) {
            Ok(lines) => {
                let mut combined = evidence.lines.clone();
                combined.extend(lines);
                printed = super::printed::read(&combined);
                if printed.is_some() { evidence.lines = combined; }
            }
            Err(_) => evidence.warnings.push("Printed set-code refinement was unavailable; the original OCR evidence is retained.".into()),
        }
    }
    if let Some(printed) = printed {
        evidence
            .extensions
            .insert("uncataloguedCard".into(), serde_json::json!(printed));
        evidence.warnings.push("Printed details read locally; no matching catalogue entry. These are OCR transcriptions, not a verified printing. Catalogue lookup will retry automatically.".into());
    }
    evidence
}

fn code_lines(
    bytes: &[u8],
    turns: u32,
    library: &Path,
    data: &Path,
    options: Options,
) -> scanner_types::error::Result<Vec<scanner_types::recognition::RecognizedLine>> {
    let image = scanner_images::decode::decode(bytes)?.to_rgb8();
    let rotated = match turns {
        1 => image::imageops::rotate90(&image),
        2 => image::imageops::rotate180(&image),
        3 => image::imageops::rotate270(&image),
        _ => image,
    };
    let mut ocr = Tesseract::new(library, data, "en")?;
    if let Some(deadline) = options.deadline {
        ocr.set_deadline(deadline);
    }
    ocr.read_set_code(&rotated, false)
}
