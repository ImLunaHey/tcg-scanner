//! Keep independent OCR passes separate: merging conflicting readings can create
//! a plausible but incorrect collector number. Selection belongs to the matcher.
use super::{RecognizedLine, Tesseract, ocr_error};
use image::{Rgb, RgbImage, imageops};
use scanner_types::error::Result;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct CardReadings {
    pub whole: Vec<RecognizedLine>,
    pub title: Vec<RecognizedLine>,
    pub footer: Vec<RecognizedLine>,
    pub left: Vec<RecognizedLine>,
    pub right: Vec<RecognizedLine>,
}
impl CardReadings {
    pub fn regional(&self) -> Vec<RecognizedLine> {
        [&self.title, &self.footer, &self.left, &self.right]
            .into_iter()
            .flatten()
            .cloned()
            .collect()
    }
}

impl Tesseract {
    /// Read an upright card and four text bands. Output boxes stay in the full
    /// image's normalized bottom-left coordinates, independent of crop/resize.
    pub fn read_card(&mut self, image: &RgbImage) -> Result<CardReadings> {
        if image.width() < 20
            || image.height() < 20
            || image.width() > 20_000
            || image.height() > 20_000
            || u64::from(image.width()) * u64::from(image.height()) > 48_000_000
        {
            return Err(ocr_error(
                "Card OCR requires 20–20000 pixel sides and at most 48 megapixels.",
            ));
        }
        let resized = (image.width().max(image.height()) > 1600)
            .then(|| crate::preprocess::thumbnail(image, 1600));
        let whole = self.recognize(resized.as_ref().unwrap_or(image), false)?;
        let title = self.read_region(image, [0., 0., 1., 0.24])?;
        let footer = self.read_region(image, [0., 0.68, 1., 0.32])?;
        let left = self.read_region(image, [0., 0.82, 0.5, 0.18])?;
        let right = self.read_region(image, [0.5, 0.82, 0.5, 0.18])?;
        Ok(CardReadings {
            whole,
            title,
            footer,
            left,
            right,
        })
    }

    fn read_region(&mut self, image: &RgbImage, region: [f64; 4]) -> Result<Vec<RecognizedLine>> {
        let crop = Region::new(image.width(), image.height(), region);
        let pixels =
            imageops::crop_imm(image, crop.left, crop.top, crop.width, crop.height).to_image();
        let height =
            ((f64::from(crop.height) / f64::from(crop.width) * 820.).round() as u32).clamp(1, 1600);
        let mut pixels = imageops::resize(&pixels, 820, height, imageops::FilterType::Lanczos3);
        for pixel in pixels.pixels_mut() {
            let [r, g, b] = pixel.0.map(f64::from);
            let value = ((r * 0.2126 + g * 0.7152 + b * 0.0722) * 1.28 + 4.)
                .round()
                .clamp(0., 255.) as u8;
            *pixel = Rgb([value; 3]);
        }
        let mut lines = self.recognize(&pixels, false)?;
        for line in &mut lines {
            crop.map(line, image.width(), image.height());
        }
        Ok(lines)
    }
}

struct Region {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
}
impl Region {
    fn new(width: u32, height: u32, [x, y, w, h]: [f64; 4]) -> Self {
        let left = (x * f64::from(width)).round() as u32;
        let top = (y * f64::from(height)).round() as u32;
        Self {
            left,
            top,
            width: ((w * f64::from(width)).floor() as u32).clamp(1, width - left),
            height: ((h * f64::from(height)).floor() as u32).clamp(1, height - top),
        }
    }
    fn map(&self, line: &mut RecognizedLine, width: u32, height: u32) {
        line.x = (f64::from(self.left) + line.x * f64::from(self.width)) / f64::from(width);
        line.y =
            1. - (f64::from(self.top) + (1. - line.y) * f64::from(self.height)) / f64::from(height);
        line.width *= f64::from(self.width) / f64::from(width);
        line.height *= f64::from(self.height) / f64::from(height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn footer_regions_keep_their_global_position_after_resizing() {
        let crop = Region::new(630, 880, [0.5, 0.82, 0.5, 0.18]);
        let mut line = RecognizedLine {
            text: "123/456".into(),
            confidence: 0.9,
            x: 0.1,
            y: 0.2,
            width: 0.5,
            height: 0.3,
        };
        crop.map(&mut line, 630, 880);
        assert!((line.x - 0.55).abs() < 1e-9);
        assert!((line.y - 0.036).abs() < 0.002);
        assert!((line.width - 0.25).abs() < 1e-9);
        assert!(line.y < 0.18 && line.height < 0.06);
    }
}
