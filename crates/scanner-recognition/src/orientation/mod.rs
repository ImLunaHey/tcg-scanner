//! Establish orientation from independent printed layout and OCR evidence.
//! Uncertain inputs retain zero turns; visual similarity alone never verifies it.
mod layout;
mod text;
use crate::ocr::{RecognizedLine, Tesseract};
use image::{RgbImage, imageops};
use scanner_types::error::{Result, ScannerError};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Orientation {
    pub quarter_turns: u8,
    pub verified: bool,
    pub basis: &'static str,
}
impl Orientation {
    fn known(turns: u8, basis: &'static str) -> Self {
        Self {
            quarter_turns: turns,
            verified: true,
            basis,
        }
    }
    fn unknown() -> Self {
        Self {
            quarter_turns: 0,
            verified: false,
            basis: "unknown",
        }
    }
}

impl Tesseract {
    pub fn orientation(&mut self, image: &RgbImage) -> Result<Orientation> {
        let (w, h) = image.dimensions();
        if w < 20 || h < 20 || w > 20_000 || h > 20_000 || u64::from(w) * u64::from(h) > 48_000_000
        {
            return Err(ScannerError::new(
                "image",
                "Orientation input exceeds supported dimensions.",
            ));
        }
        let resized = (w.max(h) > 1200).then(|| crate::preprocess::thumbnail(image, 1200));
        let image = resized.as_ref().unwrap_or(image);
        if let Some(turns) = layout::pokemon(image)
            && self.pokemon_logo(image, turns)?
        {
            return Ok(Orientation::known(turns, "pokemon-back-pattern"));
        }
        let swu = layout::starwars(image);
        let ygo = layout::yugioh(image);
        let mut readings = vec![];
        let mut known = vec![];
        for turns in 0..4 {
            let pixels = layout::rotate(image, turns);
            let lines = self.recognize(&pixels, false)?;
            let basis = if pixels.width() < pixels.height() && swu && text::starwars(&lines) {
                Some("starwars-back-pattern")
            } else if pixels.width() < pixels.height() && ygo && text::yugioh(&lines) {
                Some("yugioh-back-pattern")
            } else {
                text::code_card(&lines).or_else(|| text::energy(&lines).then_some("text"))
            };
            if let Some(basis) = basis {
                known.push(Orientation::known(turns, basis));
            }
            readings.push(lines);
        }
        if known.len() == 1 {
            return Ok(known.remove(0));
        }
        if known.len() > 1 {
            return Ok(Orientation::unknown());
        }
        Ok(from_scores(&readings))
    }
    fn pokemon_logo(&mut self, image: &RgbImage, turns: u8) -> Result<bool> {
        for rotation in [turns, (turns + 2) % 4] {
            let upright = layout::rotate(image, rotation);
            let full = self.recognize(&upright, false)?;
            if full.iter().any(logo) {
                return Ok(true);
            }
            let band = imageops::crop_imm(
                &upright,
                (f64::from(upright.width()) * 0.04) as u32,
                (f64::from(upright.height()) * 0.02) as u32,
                (f64::from(upright.width()) * 0.92) as u32,
                (f64::from(upright.height()) * 0.28) as u32,
            )
            .to_image();
            if self.recognize(&band, true)?.iter().any(logo) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
fn logo(line: &RecognizedLine) -> bool {
    line.confidence >= 0.5
        && (6..=8).contains(&text::letters(&line.text).chars().count())
        && text::resembles(&line.text, "pokemon", 2)
}
fn from_scores(readings: &[Vec<RecognizedLine>]) -> Orientation {
    let scores: Vec<_> = readings.iter().map(|l| text::score(l)).collect();
    let total: f64 = scores.iter().map(|s| s.1).sum();
    if let Some((turns, (count, score))) = scores
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.1.total_cmp(&b.1.1))
        && *count >= 4
        && *score >= 20.
        && *score >= total * 0.8
    {
        Orientation::known(turns as u8, "text")
    } else {
        Orientation::unknown()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ambiguous_direction_never_rotates_the_image() {
        let line = RecognizedLine {
            text: "Readable words".into(),
            confidence: 0.9,
            x: 0.1,
            y: 0.5,
            width: 0.6,
            height: 0.05,
        };
        let good = vec![line; 4];
        let readings = vec![good.clone(), vec![], good.clone(), vec![]];
        assert!(!from_scores(&readings).verified);
        let result = from_scores(&[vec![], good, vec![], vec![]]);
        assert!(result.verified);
        assert_eq!(result.quarter_turns, 1);
    }
}
