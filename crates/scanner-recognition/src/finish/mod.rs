//! Printed finish evidence. Suggestions never replace manual review fields.
mod background;
mod border;
mod decision;
mod marker;
pub mod stamp;
use crate::ocr::Tesseract;
use scanner_types::{
    cards::CardLanguage,
    finish::{FinishEstimate, FinishState, FinishUncertainty},
    images::CardPreparation,
    recognition::RecognitionEvidence,
};

/// Ordinary-printing evidence only. Hosts must resolve edition/stamp evidence
/// before presenting this as a final finish suggestion.
pub fn ordinary(
    bytes: &[u8],
    evidence: &RecognitionEvidence,
    preparation: Option<&CardPreparation>,
    language: CardLanguage,
    ocr: Option<&mut Tesseract>,
) -> FinishEstimate {
    let candidate = (evidence.candidates.len() == 1).then(|| evidence.candidates[0].id.clone());
    if language != CardLanguage::English {
        let mut result = FinishEstimate::new(
            FinishState::Unsupported,
            candidate,
            vec![
                "Automatic finish and stamp checks have not been validated for this card language."
                    .into(),
            ],
            Some(FinishUncertainty::LanguageUnsupported),
        );
        result.method = "language-scope-v1".into();
        return result;
    }
    marker::analyze(bytes, evidence, preparation, ocr).unwrap_or_else(|_| {
        let mut result = FinishEstimate::new(
            FinishState::Uncertain,
            candidate,
            vec!["Local finish analysis could not read the image evidence.".into()],
            Some(FinishUncertainty::AnalysisFailed),
        );
        result
            .limitations
            .push("Inspect the physical card; no finish has been confirmed.".into());
        result
    })
}
