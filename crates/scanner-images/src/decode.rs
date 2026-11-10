use crate::image_error;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits};
use scanner_types::error::{Result, ScannerError};
use std::io::Cursor;

/// Validate metadata before allocating decoded pixels; apply EXIF exactly once.
pub fn decode(bytes: &[u8]) -> Result<DynamicImage> {
    if bytes.is_empty() || bytes.len() > 32 * 1024 * 1024 {
        return Err(limit());
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(image_error)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    limits.max_alloc = Some(384 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(image_error)?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 48_000_000 {
        return Err(limit());
    }
    let orientation = decoder.orientation().map_err(image_error)?;
    let profile = decoder.icc_profile().map_err(image_error)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(image_error)?;
    image.apply_orientation(orientation);
    if let Some(profile) = profile {
        image = apply_profile(image, &profile)?;
    }
    Ok(image)
}

fn apply_profile(image: DynamicImage, profile: &[u8]) -> Result<DynamicImage> {
    use moxcms::{ColorProfile, Layout, TransformOptions};
    if profile.len() > 4 * 1024 * 1024 {
        return Err(limit());
    }
    let source = ColorProfile::new_from_slice(profile).map_err(image_error)?;
    let destination = ColorProfile::new_srgb();
    let transform = source
        .create_transform_8bit(
            Layout::Rgba,
            &destination,
            Layout::Rgba,
            TransformOptions::default(),
        )
        .map_err(image_error)?;
    let source_pixels = image.to_rgba8();
    let mut output = image::RgbaImage::new(source_pixels.width(), source_pixels.height());
    transform
        .transform(source_pixels.as_raw(), output.as_mut())
        .map_err(image_error)?;
    Ok(DynamicImage::ImageRgba8(output))
}

fn limit() -> ScannerError {
    ScannerError::new(
        "resourceLimit",
        "Image exceeds 32 MiB, 48 megapixels or 20,000 pixels per side.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_empty_and_corrupt_images() {
        assert!(decode(&[]).is_err());
        assert!(decode(b"not an image").is_err());
    }

    #[test]
    fn tagged_linear_rgb_is_converted_to_srgb_before_boundary_detection() {
        use image::{ImageEncoder, codecs::png::PngEncoder};
        use moxcms::{ColorProfile, ToneReprCurve};
        let mut profile = ColorProfile::new_srgb();
        profile.red_trc = Some(ToneReprCurve::Parametric(vec![1.]));
        profile.green_trc = profile.red_trc.clone();
        profile.blue_trc = profile.red_trc.clone();
        profile.cicp = None;
        let mut bytes = Vec::new();
        let mut encoder = PngEncoder::new(&mut bytes);
        encoder.set_icc_profile(profile.encode().unwrap()).unwrap();
        encoder
            .write_image(&[32, 32, 32], 1, 1, image::ExtendedColorType::Rgb8)
            .unwrap();
        let decoded = decode(&bytes).unwrap().to_rgb8();
        // 32/255 linear luminance becomes approximately 99/255 in sRGB.
        assert!((95..=102).contains(&decoded.get_pixel(0, 0)[0]));
    }
}
