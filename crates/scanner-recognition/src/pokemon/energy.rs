use crate::{readings::Reading, symbols::energy};
use regex::Regex;
use scanner_types::{printed::EnergySymbol, recognition::RecognitionEvidence};
use std::sync::LazyLock;

pub(super) struct ReadingSymbol {
    pub turns: u32,
    pub symbol: EnergySymbol,
    pub unnumbered: Option<RecognitionEvidence>,
}
pub(super) fn read(bytes: &[u8], readings: &[Reading]) -> Option<ReadingSymbol> {
    for turns in 0..4 {
        // Include body text from the whole-card pass even when a regional title
        // is clearer. A regional pass alone cannot exclude Special Energy rules.
        let lines: Vec<_> = readings
            .iter()
            .filter(|r| r.turns == turns)
            .flat_map(|r| r.lines.iter().cloned())
            .collect();
        if !energy::has_heading(&lines) {
            continue;
        }
        let image = crate::preprocess::thumbnail(
            &scanner_images::decode::decode(bytes).ok()?.to_rgb8(),
            2400,
        );
        let rotated = match turns {
            1 => image::imageops::rotate90(&image),
            2 => image::imageops::rotate180(&image),
            3 => image::imageops::rotate270(&image),
            _ => image,
        };
        if let Some(symbol) = energy::analyze(&rotated, &lines) {
            let unnumbered=(!has_identifier(&lines)).then(|| {
                let mut evidence=RecognitionEvidence { lines,orientation:turns,
                    warnings:vec!["Energy type recognized from the top-right symbol. Exact printing is unknown; the symbol does not establish set, price or finish.".into()],..Default::default() };
                evidence.extensions.insert("energySymbol".into(),serde_json::json!(symbol));
                evidence
            });
            return Some(ReadingSymbol {
                turns,
                symbol,
                unnumbered,
            });
        }
    }
    None
}
fn has_identifier(lines: &[scanner_types::recognition::RecognizedLine]) -> bool {
    static NUMBER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(\d\s*/\s*\d|\b(?:SVE|SWSH|SVP|SM|XY)\s*\d|^\s*\d{1,4}\s*$)").unwrap()
    });
    lines.iter().any(|l| {
        l.y < 0.15
            && l.confidence >= 0.7
            && (NUMBER.is_match(&l.text)
                || ["sve", "sveen", "mee", "meeen"].contains(
                    &crate::matching::text::normalized(&l.text)
                        .replace(' ', "")
                        .as_str(),
                ))
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn identifiers_require_readable_footer_text_not_body_artifacts() {
        let mut line = scanner_types::recognition::RecognizedLine {
            text: "2".into(),
            confidence: 0.49,
            x: 0.5,
            y: 0.285,
            width: 0.01,
            height: 0.01,
        };
        assert!(!super::has_identifier(&[line.clone()]));
        line.y = 0.04;
        line.confidence = 0.9;
        assert!(super::has_identifier(&[line.clone()]));
        line.text = "©2022 Pokémon / Nintendo / Creatures / GAME FREAK".into();
        assert!(!super::has_identifier(&[line.clone()]));
        for text in ["SVE EN", "013/191", "SM001"] {
            line.text = text.into();
            assert!(super::has_identifier(&[line.clone()]));
        }
    }
    struct NoNetwork;
    #[async_trait::async_trait]
    impl scanner_catalogue::JsonTransport for NoNetwork {
        async fn get(&self, _: &str, _: usize) -> scanner_types::error::Result<Vec<u8>> {
            panic!("Unnumbered basic Energy must not request a catalogue");
        }
    }
    #[tokio::test]
    async fn fresh_saved_energy_ocr_preserves_type_in_every_orientation_without_catalogue() {
        let Some(path) = std::env::var_os("SCANNER_ENERGY_RECOGNITION_IMAGE") else {
            return;
        };
        let library: std::path::PathBuf = std::env::var_os("SCANNER_TESSERACT_LIBRARY")
            .unwrap()
            .into();
        let data: std::path::PathBuf = std::env::var_os("SCANNER_TESSERACT_DATA").unwrap().into();
        let mut recognizer =
            super::super::PokemonRecognizer::new(library.clone(), data.clone()).unwrap();
        recognizer.english = scanner_catalogue::TcgDex::with_transport(
            scanner_types::cards::CardLanguage::English,
            std::sync::Arc::new(NoNetwork),
        );
        let pixels = scanner_images::decode::decode(&std::fs::read(path).unwrap())
            .unwrap()
            .to_rgb8();
        for turns in 0..4 {
            let image = match turns {
                1 => image::imageops::rotate90(&pixels),
                2 => image::imageops::rotate180(&pixels),
                3 => image::imageops::rotate270(&pixels),
                _ => pixels.clone(),
            };
            let mut bytes = std::io::Cursor::new(vec![]);
            image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
            let result = recognizer
                .recognize(
                    bytes.into_inner(),
                    scanner_types::cards::CardLanguage::English,
                )
                .await
                .unwrap();
            eprintln!(
                "Energy input turns={turns}, result turns={}, symbol={:?}",
                result.orientation,
                result.extensions.get("energySymbol")
            );
            assert_eq!(result.extensions["energySymbol"]["type"], "Psychic");
            assert!(result.candidates.is_empty());
            assert_eq!(result.orientation, (4 - turns) % 4);
        }
    }
}
