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
    let mut image = DynamicImage::from_decoder(decoder).map_err(image_error)?;
    image.apply_orientation(orientation);
    Ok(image)
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
}
