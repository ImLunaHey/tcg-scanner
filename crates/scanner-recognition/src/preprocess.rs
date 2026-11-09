use image::{RgbImage, imageops};

/// Downsample for evidence analysis, retaining aspect ratio and never enlarging.
pub(crate) fn thumbnail(image: &RgbImage, max_edge: u32) -> RgbImage {
    let (w, h) = image.dimensions();
    if w.max(h) <= max_edge || w.min(h) == 0 {
        return image.clone();
    }
    let scale = f64::from(max_edge.max(1)) / f64::from(w.max(h));
    imageops::thumbnail(
        image,
        (f64::from(w) * scale).round().max(1.) as u32,
        (f64::from(h) * scale).round().max(1.) as u32,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portrait_landscape_and_small_images_keep_their_shape() {
        for (source, expected) in [
            ((630, 880), (183, 256)),
            ((880, 630), (256, 183)),
            ((63, 88), (63, 88)),
        ] {
            let pixels = RgbImage::new(source.0, source.1);
            assert_eq!(thumbnail(&pixels, 256).dimensions(), expected);
        }
    }
}
