//! Ephemeral JPEG extraction: bounded decode, a complete card outline and perspective correction.
mod quadrilateral;
use crate::{EncodedImage, image_error};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, Rgb, RgbImage};
use imageproc::geometric_transformations::{Border, Interpolation, Projection, warp_into};
use scanner_types::error::{Result, ScannerError};
use std::io::Cursor;

pub use scanner_types::camera::MAX_IMAGE_BYTES;

/// Returns `None` for a frame without a complete, sufficiently large card outline.
/// Pixels are never written to disk. The output is lossless for subsequent OCR.
pub fn extract(bytes: &[u8]) -> Result<Option<EncodedImage>> {
    let image = decode(bytes)?;
    let image = if image.width().max(image.height()) > 1600 {
        image.thumbnail(1600, 1600)
    } else {
        image
    }
    .to_rgb8();
    let analysis = DynamicImage::ImageRgb8(image.clone());
    let analysis = if analysis.width().max(analysis.height()) > 800 {
        analysis.thumbnail(800, 800)
    } else {
        analysis
    }
    .to_luma8();
    let Some(quad) = quadrilateral::find(&analysis) else {
        return Ok(None);
    };
    let sx = image.width() as f32 / analysis.width() as f32;
    let sy = image.height() as f32 / analysis.height() as f32;
    let points = quad.map(|p| (p.0 * sx, p.1 * sy));
    let corrected = rectify(&image, points)?;
    crate::encode_png(DynamicImage::ImageRgb8(corrected).to_rgba8()).map(Some)
}

fn decode(bytes: &[u8]) -> Result<DynamicImage> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(invalid());
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(image_error)?;
    if reader.format() != Some(ImageFormat::Jpeg) {
        return Err(invalid());
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(96 * 1024 * 1024);
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(image_error)?;
    let (width, height) = decoder.dimensions();
    if !(240..=4096).contains(&width)
        || !(240..=4096).contains(&height)
        || u64::from(width) * u64::from(height) > 8_000_000
    {
        return Err(invalid());
    }
    // The shared decoder applies EXIF and embedded colour profiles exactly once.
    crate::decode::decode(bytes)
}
fn rectify(image: &RgbImage, points: [(f32, f32); 4]) -> Result<RgbImage> {
    let distance =
        |a: usize, b: usize| (points[a].0 - points[b].0).hypot(points[a].1 - points[b].1);
    let width = distance(0, 1).max(distance(2, 3)).ceil() as u32;
    let height = distance(1, 2).max(distance(3, 0)).ceil() as u32;
    let destination = [
        (0., 0.),
        ((width - 1) as f32, 0.),
        ((width - 1) as f32, (height - 1) as f32),
        (0., (height - 1) as f32),
    ];
    let projection = Projection::from_control_points(points, destination)
        .ok_or_else(|| ScannerError::new("image", "Camera perspective correction failed."))?;
    let mut output = RgbImage::new(width, height);
    warp_into(
        image,
        projection,
        Interpolation::Bilinear,
        Border::Constant(Rgb([0, 0, 0])),
        &mut output,
    );
    Ok(output)
}
fn invalid() -> ScannerError {
    ScannerError::new(
        "invalidParams",
        "Provide one JPEG up to 512 KiB, 240–4096 pixels per side and 8 megapixels.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use imageproc::{drawing::draw_polygon_mut, point::Point};
    fn jpeg(image: &RgbImage) -> Vec<u8> {
        let mut bytes = vec![];
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 92)
            .encode_image(image)
            .unwrap();
        bytes
    }
    #[test]
    fn rectifies_perspective_without_mirroring_or_retaining_background() {
        let mut frame = RgbImage::from_pixel(600, 800, Rgb([25, 25, 25]));
        let corners = [(100, 60), (510, 100), (480, 750), (70, 700)].map(|(x, y)| Point::new(x, y));
        draw_polygon_mut(&mut frame, &corners, Rgb([230, 230, 210]));
        // Asymmetric marks verify that extraction doesn't mirror the card.
        imageproc::drawing::draw_filled_circle_mut(&mut frame, (160, 150), 25, Rgb([240, 30, 30]));
        imageproc::drawing::draw_filled_circle_mut(&mut frame, (420, 650), 25, Rgb([30, 30, 240]));
        let card = extract(&jpeg(&frame)).unwrap().unwrap();
        assert!((400..=425).contains(&card.width), "{}", card.width);
        assert!((640..=665).contains(&card.height), "{}", card.height);
        let pixels = crate::decode::decode(&card.bytes).unwrap().to_rgb8();
        let red = pixels.get_pixel(65, 85);
        let blue = pixels.get_pixel(348, 551);
        assert!(red[0] > 180 && red[2] < 80, "{red:?}");
        assert!(blue[2] > 180 && blue[0] < 80, "{blue:?}");
    }
    #[test]
    fn refuses_non_jpeg_oversized_metadata_and_empty_scenes() {
        assert!(extract(b"not a jpeg").is_err());
        assert!(extract(&vec![0; MAX_IMAGE_BYTES + 1]).is_err());
        for (w, h) in [(239, 400), (4097, 300), (3000, 3000)] {
            assert!(extract(&jpeg(&RgbImage::new(w, h))).is_err());
        }
        assert!(extract(&jpeg(&RgbImage::new(600, 800))).unwrap().is_none());
        let png = crate::encode_png(image::RgbaImage::new(600, 800)).unwrap();
        assert!(extract(&png.bytes).is_err());
    }
}
