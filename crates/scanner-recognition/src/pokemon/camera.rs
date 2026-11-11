//! Cheap upright identity attempt. Uncertain fields fall back to the full recognizer.
use super::{PokemonRecognizer, error, resolve};
use crate::{ocr::Tesseract, readings::Reading};
use scanner_types::{cards::CardLanguage, error::Result, recognition::RecognitionEvidence};
use std::time::Instant;

impl PokemonRecognizer {
    pub(crate) async fn camera_identity(
        &self,
        bytes: Vec<u8>,
        language: CardLanguage,
        deadline: Instant,
    ) -> Result<Option<RecognitionEvidence>> {
        let permit = self.workers.clone().acquire_owned().await.map_err(error)?;
        let (library, data) = (self.library.clone(), self.data.clone());
        let readings = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let pixels = scanner_images::decode::decode(&bytes)?.to_rgb8();
            let mut ocr = Tesseract::new(&library, &data, language.as_str())?;
            ocr.set_deadline(deadline);
            ocr.read_identity(&pixels)
        })
        .await
        .map_err(error)??;
        if readings.whole.is_empty() && readings.title.is_empty() && readings.footer.is_empty() {
            return Ok(None);
        }
        let catalogue = match language {
            CardLanguage::English => &self.english,
            CardLanguage::Korean => &self.korean,
        };
        let evidence = resolve(
            vec![
                Reading {
                    turns: 0,
                    lines: readings.regional(),
                },
                Reading {
                    turns: 0,
                    lines: readings.whole,
                },
            ],
            language,
            catalogue,
        )
        .await?;
        Ok(supported(&evidence).then_some(evidence))
    }
}
fn supported(evidence: &RecognitionEvidence) -> bool {
    let [candidate] = evidence.candidates.as_slice() else {
        return false;
    };
    // All three independent printed identity fields must support this one printing.
    [
        "Name in title area",
        "Collector number ",
        "Printed set total ",
    ]
    .iter()
    .all(|prefix| candidate.evidence.iter().any(|e| e.starts_with(prefix)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn early_return_requires_one_printing_and_all_three_identity_fields() {
        let candidate = serde_json::from_value(serde_json::json!({
            "id":"fixture-1", "name":"Fixture", "localID":"1", "setID":"fixture",
            "setName":"Fixture", "evidence":["Name in title area", "Collector number 1", "Printed set total 100"]
        })).unwrap();
        let mut evidence = RecognitionEvidence {
            candidates: vec![candidate],
            ..Default::default()
        };
        assert!(supported(&evidence));
        for field in 0..3 {
            let mut incomplete = evidence.clone();
            incomplete.candidates[0].evidence.remove(field);
            assert!(!supported(&incomplete));
        }
        evidence.candidates.push(evidence.candidates[0].clone());
        assert!(!supported(&evidence));
        evidence.candidates.clear();
        assert!(!supported(&evidence));
    }
}
