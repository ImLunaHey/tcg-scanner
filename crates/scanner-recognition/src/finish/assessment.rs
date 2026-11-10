use super::{border, edition, marker, ordinary, stamp};
use crate::ocr::Tesseract;
use scanner_types::{
    cards::CardLanguage,
    edition::{EditionEvidence, EditionState, PrintingVariant, StampEvidence, StampState},
    error::Result,
    finish::{BorderState, FinishEstimate, FinishState, FinishUncertainty as Reason},
    images::CardPreparation,
    recognition::RecognitionEvidence,
};

/// Conservative complete local assessment. A stamp failure must never silently
/// substitute ordinary-printing finish metadata. No field confirms a finish.
pub fn estimate(
    bytes: &[u8],
    evidence: &RecognitionEvidence,
    preparation: Option<&CardPreparation>,
    language: CardLanguage,
    ocr: Option<&mut Tesseract>,
    camera: bool,
) -> FinishEstimate {
    if language != CardLanguage::English {
        return ordinary(bytes, evidence, preparation, language, ocr);
    }
    analyze(bytes, evidence, preparation, language, ocr, camera)
        .unwrap_or_else(|_| failed(evidence))
}
fn analyze(
    bytes: &[u8],
    evidence: &RecognitionEvidence,
    preparation: Option<&CardPreparation>,
    language: CardLanguage,
    ocr: Option<&mut Tesseract>,
    camera: bool,
) -> Result<FinishEstimate> {
    let prepared = preparation
        .is_some_and(|p| p.orientation_verified && p.orientation_basis.as_deref() == Some("text"));
    let border_preparation = if camera { None } else { preparation };
    if !prepared {
        return Ok(ordinary(bytes, evidence, border_preparation, language, ocr));
    }
    let stamp = stamp::analyze(bytes, evidence.orientation)?;
    let card = if evidence.candidates.len() == 1 {
        evidence.candidates.first()
    } else {
        None
    };
    if matches!(stamp.state, StampState::Absent | StampState::Unsupported) {
        let mut result = ordinary(bytes, evidence, border_preparation, language, ocr);
        result.edition = Some(EditionEvidence::new(
            if stamp.state == StampState::Absent {
                EditionState::NotDetected
            } else {
                EditionState::Unavailable
            },
            card.map(|c| c.id.clone()),
            stamp,
            None,
            vec![],
            vec![],
        ));
        return Ok(result);
    }
    let variants = card
        .and_then(|c| c.extensions.get("printingVariants"))
        .and_then(|v| serde_json::from_value::<Vec<PrintingVariant>>(v.clone()).ok())
        .unwrap_or_default();
    let edition = edition::resolve(stamp, card, &variants);
    let border = if !camera
        && edition::needs_border(&edition)
        && card.is_some_and(|c| marker::modern_set(&c.set_id))
    {
        Some(
            marker::upright(bytes, evidence.orientation)
                .map(|p| border::analyze(&p))
                .unwrap_or_else(|_| border::unavailable(BorderState::Failed)),
        )
    } else {
        None
    };
    Ok(edition::decision(edition, border))
}
fn failed(evidence: &RecognitionEvidence) -> FinishEstimate {
    let candidate = (evidence.candidates.len() == 1).then(|| evidence.candidates[0].id.clone());
    let mut result = FinishEstimate::new(
        FinishState::Uncertain,
        candidate.clone(),
        vec!["Local stamp/finish analysis could not complete.".into()],
        Some(Reason::AnalysisFailed),
    );
    let stamp = StampEvidence {
        state: StampState::Uncertain,
        kind: "pikachu-pumpkin".into(),
        method: "pumpkin-shape-rust-v1".into(),
        source_quarter_turns: evidence.orientation,
        region: None,
        similarity: None,
        baseline_similarity: None,
        downsampled_similarity: None,
        evidence: vec![
            "Stamp analysis was unavailable; no ordinary-printing assumption was made.".into(),
        ],
    };
    result.edition = Some(EditionEvidence::new(
        EditionState::Unavailable,
        candidate,
        stamp,
        None,
        vec![],
        vec![],
    ));
    result
}
