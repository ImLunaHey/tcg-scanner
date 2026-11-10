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
        validate(image)?;
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

    /// Independent pass over the standard YGO title, set-code and passcode
    /// fields. Keep these lines separate from whole-card/regional readings.
    pub(crate) fn read_yugioh(&mut self, image: &RgbImage) -> Result<Vec<RecognizedLine>> {
        validate(image)?;
        let mut lines = vec![];
        for field in [
            [0.04, 0.045, 0.79, 0.085],
            [0.68, 0.710, 0.25, 0.025],
            [0.035, 0.947, 0.30, 0.026],
        ] {
            lines.extend(self.read_region_with_mode(image, field, true)?);
        }
        Ok(lines)
    }
    fn read_region(&mut self, image: &RgbImage, region: [f64; 4]) -> Result<Vec<RecognizedLine>> {
        self.read_region_with_mode(image, region, false)
    }
    /// A tight title pass excludes the icon and most textured Energy artwork.
    pub(crate) fn read_energy_heading(&mut self, image: &RgbImage) -> Result<Vec<RecognizedLine>> {
        self.read_scaled_region(image, [0.035, 0.03, 0.60, 0.055], true, 400, false)
    }
    /// Individual word bounds prevent an adjacent rarity mark from becoming
    /// part of an estimated character rectangle.
    pub fn read_footer_words(&mut self, image: &RgbImage) -> Result<Vec<RecognizedLine>> {
        validate(image)?;
        self.read_scaled_region(image, [0., 0.85, 1., 0.15], false, 1600, true)
    }
    fn read_region_with_mode(
        &mut self,
        image: &RgbImage,
        region: [f64; 4],
        single_block: bool,
    ) -> Result<Vec<RecognizedLine>> {
        self.read_scaled_region(image, region, single_block, 820, false)
    }
    /// Read the lower-left printed set-code region with individual word bounds.
    pub fn read_set_code(
        &mut self,
        image: &RgbImage,
        magnify: bool,
    ) -> Result<Vec<RecognizedLine>> {
        validate(image)?;
        let mut words = self.read_scaled_region(
            image,
            [0.03, 0.915, 0.21, 0.08],
            true,
            if magnify { 1640 } else { 820 },
            true,
        )?;
        words.extend(super::set_code::joined_words(&words));
        Ok(words)
    }
    fn read_scaled_region(
        &mut self,
        image: &RgbImage,
        region: [f64; 4],
        single_block: bool,
        target_width: u32,
        words: bool,
    ) -> Result<Vec<RecognizedLine>> {
        let crop = Region::new(image.width(), image.height(), region);
        let pixels =
            imageops::crop_imm(image, crop.left, crop.top, crop.width, crop.height).to_image();
        let height = ((f64::from(crop.height) / f64::from(crop.width) * f64::from(target_width))
            .round() as u32)
            .clamp(1, 1600);
        let mut pixels = imageops::resize(
            &pixels,
            target_width,
            height,
            imageops::FilterType::Lanczos3,
        );
        for pixel in pixels.pixels_mut() {
            let [r, g, b] = pixel.0.map(f64::from);
            let value = ((r * 0.2126 + g * 0.7152 + b * 0.0722) * 1.28 + 4.)
                .round()
                .clamp(0., 255.) as u8;
            *pixel = Rgb([value; 3]);
        }
        let padded = single_block.then(|| {
            let mut out =
                RgbImage::from_pixel(pixels.width() + 32, pixels.height() + 32, Rgb([255; 3]));
            imageops::replace(&mut out, &pixels, 16, 16);
            out
        });
        let mut lines = if words {
            self.recognize_words(padded.as_ref().unwrap_or(&pixels), single_block)?
        } else if single_block {
            self.recognize(padded.as_ref().unwrap(), true)?
        } else {
            self.recognize(&pixels, false)?
        };
        for line in &mut lines {
            if single_block {
                line.x =
                    (line.x * f64::from(pixels.width() + 32) - 16.) / f64::from(pixels.width());
                line.y =
                    (line.y * f64::from(pixels.height() + 32) - 16.) / f64::from(pixels.height());
                line.width *= f64::from(pixels.width() + 32) / f64::from(pixels.width());
                line.height *= f64::from(pixels.height() + 32) / f64::from(pixels.height());
            }
            crop.map(line, image.width(), image.height());
        }
        Ok(lines)
    }
}

fn validate(image: &RgbImage) -> Result<()> {
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
    Ok(())
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
