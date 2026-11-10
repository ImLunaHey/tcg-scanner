//! Validate the service's bounded JPEG preview before handing it to the UI decoder.
use base64::{Engine, engine::general_purpose::STANDARD};
use scanner_types::{
    error::{Result, ScannerError},
    image_access::ImageThumbnail,
};
use sha2::{Digest, Sha256};
pub fn decode(thumbnail: ImageThumbnail) -> Result<Vec<u8>> {
    if thumbnail.schema_version != 1
        || thumbnail.media_type != "image/jpeg"
        || !(1..=512).contains(&thumbnail.width)
        || !(1..=512).contains(&thumbnail.height)
        || thumbnail.base64.len() > 700_000
    {
        return Err(invalid());
    }
    let bytes = STANDARD.decode(thumbnail.base64).map_err(|_| invalid())?;
    if bytes.len() > 512 * 1024
        || format!("{:x}", Sha256::digest(&bytes)) != thumbnail.sha256
        || dimensions(&bytes) != Some((thumbnail.width, thumbnail.height))
    {
        return Err(invalid());
    }
    Ok(bytes)
}
fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if !bytes.starts_with(b"\xff\xd8") {
        return None;
    }
    let mut offset = 2;
    for _ in 0..256 {
        if *bytes.get(offset)? != 0xff {
            return None;
        }
        while bytes.get(offset) == Some(&0xff) {
            offset += 1;
        }
        let marker = *bytes.get(offset)?;
        offset += 1;
        if matches!(marker, 0 | 0xd8..=0xda) {
            return None;
        }
        let length = u16::from_be_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?) as usize;
        if length < 2 {
            return None;
        }
        let segment = bytes.get(offset..offset + length)?;
        if matches!(marker, 0xc0 | 0xc2) {
            if segment.len() < 8 {
                return None;
            }
            return Some((
                u16::from_be_bytes([segment[5], segment[6]]) as u32,
                u16::from_be_bytes([segment[3], segment[4]]) as u32,
            ));
        }
        offset += length;
    }
    None
}
fn invalid() -> ScannerError {
    ScannerError::new("image", "Invalid catalogue reference preview.")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn service_jpeg_passes_but_false_dimensions_or_content_do_not() {
        let original = scanner_images::encode_png(image::RgbaImage::from_pixel(
            30,
            40,
            image::Rgba([20, 40, 60, 255]),
        ))
        .unwrap();
        let encoded = scanner_images::thumbnail::thumbnail(&original.bytes, 128, true).unwrap();
        let thumbnail = ImageThumbnail {
            schema_version: 1,
            base64: STANDARD.encode(&encoded.bytes),
            sha256: encoded.sha256,
            media_type: "image/jpeg".into(),
            width: encoded.width,
            height: encoded.height,
        };
        assert_eq!(decode(thumbnail.clone()).unwrap(), encoded.bytes);
        let mut wrong = thumbnail.clone();
        wrong.width = 31;
        assert!(decode(wrong).is_err());
        let mut corrupt = thumbnail;
        corrupt.sha256 = "0".repeat(64);
        assert!(decode(corrupt).is_err());
    }
    #[test]
    fn dimensions_are_read_from_a_bounded_frame_without_decoding_pixels() {
        let mut bytes = vec![
            0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc0, 0, 8, 8, 0, 40, 0, 30, 3,
        ];
        assert_eq!(dimensions(&bytes), Some((30, 40)));
        bytes[11] = 1;
        assert_eq!(dimensions(&bytes), None);
        assert_eq!(dimensions(&[0xff, 0xd8, 0xff, 0xda, 0, 2]), None);
        assert_eq!(dimensions(&[0xff, 0xd8, 0xff, 0xc0, 0, 8]), None);
    }
}
