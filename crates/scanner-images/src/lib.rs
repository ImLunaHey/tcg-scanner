pub mod boundary;
pub mod camera;
pub mod decode;
pub mod deskew;
pub mod edit;
pub mod thumbnail;

use image::RgbaImage;
use scanner_types::error::{Result, ScannerError};
use sha2::{Digest, Sha256};

pub struct EncodedImage {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub sha256: String,
    pub media_type: &'static str,
}

pub fn encode_png(image: RgbaImage) -> Result<EncodedImage> {
    use image::{ImageEncoder, codecs::png::PngEncoder};
    let (width, height) = image.dimensions();
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(
            image.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(image_error)?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(ScannerError::new(
            "resourceLimit",
            "Encoded image exceeds 64 MiB.",
        ));
    }
    Ok(EncodedImage {
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        bytes,
        width,
        height,
        media_type: "image/png",
    })
}

pub(crate) fn image_error(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("image", error.to_string())
}
