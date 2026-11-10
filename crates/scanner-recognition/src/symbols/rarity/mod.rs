mod evidence;
mod shape;
pub use evidence::apply;
use image::RgbImage;
use regex::Regex;
use scanner_types::{images::ImageCrop, printed::RaritySymbol, recognition::RecognizedLine};
use std::sync::LazyLock;

/// Word bounds must contain exactly the numeric fraction. Do not estimate
/// character positions inside an OCR line containing a merged rarity glyph.
pub fn analyze(upright: &RgbImage, words: &[RecognizedLine]) -> Option<RaritySymbol> {
    static FRACTION: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^([0-9]{1,4})\s*/\s*([0-9]{1,4})$").unwrap());
    if upright.height() < 1000 {
        return None;
    }
    let anchors: Vec<_> = words
        .iter()
        .filter(|w| w.confidence >= 0.7 && w.y < 0.15)
        .filter_map(|w| {
            FRACTION
                .captures(w.text.trim())
                .map(|c| (w, c[1].to_owned(), c[2].to_owned()))
        })
        .collect();
    if anchors.len() != 1 {
        return None;
    }
    let (number, local, total) = &anchors[0];
    if !(0.003..0.025).contains(&number.height) || number.x + number.width >= 0.95 {
        return None;
    }
    let region = ImageCrop {
        x: number.x + number.width + 0.001,
        y: 1. - number.y - number.height * 1.35,
        width: 0.065f64.min(1. - number.x - number.width - 0.001),
        height: number.height * 1.7,
    };
    let crop = crop(upright, region)?;
    let light = crop
        .pixels()
        .filter(|p| p.0.iter().map(|c| u32::from(*c)).sum::<u32>() > 270)
        .count();
    if light as f64 / (crop.width() * crop.height()) as f64 <= 0.55 {
        return None;
    }
    let a = shape::classify(&crop, 90)?;
    let b = shape::classify(&crop, 135)?;
    if a.0 != b.0 {
        return None;
    }
    Some(RaritySymbol {
        method: "black-footer-shape-v1".into(),
        symbol: a.0,
        similarity: a.1.min(b.1),
        region,
        collector_number: local.clone(),
        set_total: total.clone(),
    })
}
fn crop(image: &RgbImage, r: ImageCrop) -> Option<RgbImage> {
    if [r.x, r.y, r.width, r.height]
        .iter()
        .any(|n| !n.is_finite() || *n < 0.)
        || r.x + r.width > 1.
        || r.y + r.height > 1.
    {
        return None;
    }
    let (x, y) = (
        (r.x * image.width() as f64).floor() as u32,
        (r.y * image.height() as f64).floor() as u32,
    );
    let (right, bottom) = (
        ((r.x + r.width) * image.width() as f64).ceil() as u32,
        ((r.y + r.height) * image.height() as f64).ceil() as u32,
    );
    let (w, h) = (
        right.min(image.width()).checked_sub(x)?,
        bottom.min(image.height()).checked_sub(y)?,
    );
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) >= 100_000 {
        return None;
    }
    Some(image::imageops::crop_imm(image, x, y, w, h).to_image())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uses_fraction_word_bounds_and_rejects_merged_symbols_or_ambiguous_anchors() {
        let mut image = RgbImage::from_pixel(1400, 2000, image::Rgb([255; 3]));
        for y in 1897..1920 {
            for x in 257..280 {
                if (x as i32 - 268).pow(2) + (y as i32 - 1908).pow(2) <= 100 {
                    image.put_pixel(x, y, image::Rgb([0; 3]));
                }
            }
        }
        let mut word = RecognizedLine {
            text: "001/100".into(),
            confidence: 0.95,
            x: 0.10,
            y: 0.04,
            width: 0.08,
            height: 0.012,
        };
        let result = analyze(&image, &[word.clone()]).unwrap();
        assert_eq!(result.symbol, "circle");
        assert_eq!(result.collector_number, "001");
        assert!(analyze(&image, &[word.clone(), word.clone()]).is_none());
        word.text = "001/100 ★".into();
        assert!(analyze(&image, &[word.clone()]).is_none());
        word.text = "001/100".into();
        word.confidence = 0.69;
        assert!(analyze(&image, &[word]).is_none());
    }
}
