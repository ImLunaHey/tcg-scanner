use crate::{EncodedImage, decode::decode, encode_png};
use image::{Rgba, RgbaImage, imageops};
use imageproc::geometric_transformations::{Border, Interpolation, Projection, warp_into};
use scanner_types::{
    error::{Result, ScannerError},
    images::ImageEditRequest,
};

pub fn render(bytes: &[u8], request: &ImageEditRequest) -> Result<EncodedImage> {
    request.validate()?;
    let source = decode(bytes)?.to_rgba8();
    encode_png(render_pixels(&source, request)?)
}

/// Crop coordinates refer to the expanded, deskewed canvas, before quarter turns.
pub fn render_pixels(source: &RgbaImage, request: &ImageEditRequest) -> Result<RgbaImage> {
    request.validate()?;
    let (w, h) = source.dimensions();
    let angle = request.deskew_degrees.unwrap_or(0.).to_radians();
    let bounds = crate::deskew::enclosure(
        &[
            (0., 0.),
            (w as f64, 0.),
            (0., h as f64),
            (w as f64, h as f64),
        ],
        angle,
    );
    let c = request.crop;
    let left = (c.x * bounds.width()).floor();
    let top = (c.y * bounds.height()).floor();
    let right = ((c.x + c.width) * bounds.width())
        .ceil()
        .min(bounds.width().ceil());
    let bottom = ((c.y + c.height) * bounds.height())
        .ceil()
        .min(bounds.height().ceil());
    let width = (right - left) as u32;
    let height = (bottom - top) as u32;
    if width == 0
        || height == 0
        || width > 20_000
        || height > 20_000
        || u64::from(width) * u64::from(height) > 40_000_000
    {
        return Err(ScannerError::new(
            "resourceLimit",
            "Edited image exceeds bounds or is smaller than one pixel.",
        ));
    }
    let cropped = if angle == 0. {
        imageops::crop_imm(
            source,
            left as u32,
            top as u32,
            width.min(w - left as u32),
            height.min(h - top as u32),
        )
        .to_image()
    } else {
        let projection = Projection::translate(0.5, 0.5)
            .and_then(Projection::rotate(angle as f32))
            .and_then(Projection::translate(
                (-bounds.left - left - 0.5) as f32,
                (-bounds.top - top - 0.5) as f32,
            ));
        let mut out = RgbaImage::new(width, height);
        warp_into(
            source,
            projection,
            Interpolation::Bilinear,
            Border::Constant(Rgba([0, 0, 0, 0])),
            &mut out,
        );
        out
    };
    Ok(match request.quarter_turns {
        1 => imageops::rotate90(&cropped),
        2 => imageops::rotate180(&cropped),
        3 => imageops::rotate270(&cropped),
        _ => cropped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use scanner_types::{Extensions, images::ImageCrop};
    use uuid::Uuid;
    #[test]
    fn crop_then_clockwise_rotation_preserves_corner_pixels() {
        let mut source = RgbaImage::from_pixel(6, 4, Rgba([0, 0, 0, 255]));
        source.put_pixel(2, 0, Rgba([255, 0, 0, 255]));
        source.put_pixel(5, 3, Rgba([0, 255, 0, 255]));
        let request = ImageEditRequest {
            id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            sequence: 1,
            crop: ImageCrop {
                x: 1. / 3.,
                y: 0.,
                width: 2. / 3.,
                height: 1.,
            },
            quarter_turns: 1,
            deskew_degrees: None,
            extensions: Extensions::new(),
        };
        let output = render_pixels(&source, &request).unwrap();
        assert_eq!(output.dimensions(), (4, 4));
        assert_eq!(output.get_pixel(3, 0), &Rgba([255, 0, 0, 255]));
        assert_eq!(output.get_pixel(0, 3), &Rgba([0, 255, 0, 255]));
        assert_eq!(source.get_pixel(2, 0), &Rgba([255, 0, 0, 255]));
    }
}
