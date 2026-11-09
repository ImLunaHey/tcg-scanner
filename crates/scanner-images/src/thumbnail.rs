use crate::{EncodedImage, decode::decode, encode_png, image_error};
use image::codecs::jpeg::JpegEncoder;
use scanner_types::error::{Result, ScannerError};
use sha2::{Digest, Sha256};

pub fn thumbnail(bytes: &[u8], max_edge: u32, jpeg: bool) -> Result<EncodedImage> {
    if !(32..=2048).contains(&max_edge) {
        return Err(ScannerError::new(
            "invalidParams",
            "Thumbnail size must be 32–2048 pixels.",
        ));
    }
    let image = decode(bytes)?.thumbnail(max_edge, max_edge);
    let result = if jpeg {
        let mut bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut bytes, 82)
            .encode_image(&image.to_rgb8())
            .map_err(image_error)?;
        EncodedImage {
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            bytes,
            width: image.width(),
            height: image.height(),
            media_type: "image/jpeg",
        }
    } else {
        encode_png(image.to_rgba8())?
    };
    if result.bytes.len() > 2 * 1024 * 1024 {
        return Err(ScannerError::new(
            "resourceLimit",
            "Thumbnail exceeds 2 MiB.",
        ));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thumbnail_keeps_aspect_and_is_a_real_decodable_jpeg() {
        let source = encode_png(image::RgbaImage::from_pixel(
            400,
            600,
            image::Rgba([12, 25, 90, 255]),
        ))
        .unwrap();
        let thumb = thumbnail(&source.bytes, 200, true).unwrap();
        assert_eq!(thumb.height, 200);
        assert!((thumb.width as f64 / thumb.height as f64 - 2. / 3.).abs() < 0.01);
        assert_eq!(decode(&thumb.bytes).unwrap().height(), 200);
    }
}
