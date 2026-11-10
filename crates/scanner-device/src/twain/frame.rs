use super::{
    manager::error,
    transfer_abi::{ImageInfo, ImageTransfer},
};
use image::{DynamicImage, RgbImage};
use scanner_types::error::Result;

pub(super) struct Frame {
    width: u32,
    height: Option<u32>,
    rows: u32,
    pixels: Vec<u8>,
}
impl Frame {
    pub fn new(info: ImageInfo) -> Result<Self> {
        let bits = info.bits;
        if info.samples != 3
            || info.bits_per_pixel != 24
            || bits[..3] != [8, 8, 8]
            || info.planar != 0
            || info.pixel_type != 2
            || info.compression != 0
        {
            return Err(error(
                "TWAIN must return uncompressed interleaved RGB8 images.",
            ));
        }
        let width = info.width;
        let height = info.height;
        if !(1..=20000).contains(&width) || !(1..=20000).contains(&height) && info.height != -1 {
            return Err(error("Invalid TWAIN image dimensions."));
        }
        if info.height > 0 && info.width as u64 * info.height as u64 > 48_000_000 {
            return Err(error("TWAIN image exceeds pixel limits."));
        }
        Ok(Self {
            width: info.width as u32,
            height: (info.height != -1).then_some(info.height as u32),
            rows: 0,
            pixels: Vec::new(),
        })
    }
    pub fn append(&mut self, transfer: &ImageTransfer, bytes: &[u8]) -> Result<bool> {
        let count = transfer.bytes_written as usize;
        if count > bytes.len() || transfer.compression != 0 {
            return Err(error("Invalid TWAIN transfer length or compression."));
        }
        if count == 0 && transfer.rows == 0 {
            return Ok(false);
        }
        if transfer.x_offset != 0
            || transfer.y_offset != self.rows
            || transfer.columns != self.width
            || transfer.rows == 0
        {
            return Err(error("TWAIN must deliver contiguous full-width strips."));
        }
        let rows = self
            .rows
            .checked_add(transfer.rows)
            .ok_or_else(|| error("TWAIN row count overflow."))?;
        if rows > 20000
            || rows as u64 * self.width as u64 > 48_000_000
            || self.height.is_some_and(|height| rows > height)
        {
            return Err(error("TWAIN frame exceeds declared or maximum dimensions."));
        }
        let packed = self.width as usize * 3;
        let stride = transfer.stride as usize;
        if stride < packed || stride > packed + 65536 {
            return Err(error("Invalid TWAIN strip stride."));
        }
        let needed = (transfer.rows as usize - 1)
            .checked_mul(stride)
            .and_then(|n| n.checked_add(packed))
            .ok_or_else(|| error("TWAIN strip size overflow."))?;
        if count < needed || count > transfer.rows as usize * stride {
            return Err(error("Truncated TWAIN strip."));
        }
        self.pixels
            .try_reserve(transfer.rows as usize * packed)
            .map_err(error)?;
        for row in 0..transfer.rows as usize {
            self.pixels
                .extend_from_slice(&bytes[row * stride..row * stride + packed]);
        }
        self.rows = rows;
        Ok(true)
    }
    pub fn finish(self) -> Result<DynamicImage> {
        if self.rows == 0 || self.height.is_some_and(|height| height != self.rows) {
            return Err(error("TWAIN frame ended before all rows arrived."));
        }
        let image = RgbImage::from_raw(self.width, self.rows, self.pixels)
            .ok_or_else(|| error("Invalid TWAIN RGB buffer."))?;
        Ok(DynamicImage::ImageRgb8(image))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn info(height: i32) -> ImageInfo {
        ImageInfo {
            width: 2,
            height,
            samples: 3,
            bits: [8, 8, 8, 0, 0, 0, 0, 0],
            bits_per_pixel: 24,
            pixel_type: 2,
            ..ImageInfo::default()
        }
    }
    fn strip(y: u32) -> ImageTransfer {
        ImageTransfer {
            stride: 8,
            columns: 2,
            rows: 1,
            y_offset: y,
            bytes_written: 8,
            ..ImageTransfer::default()
        }
    }
    #[test]
    fn keeps_rgb_and_strips_padding_for_known_or_unknown_height() {
        for height in [2, -1] {
            let mut frame = Frame::new(info(height)).unwrap();
            frame
                .append(&strip(0), &[1, 2, 3, 4, 5, 6, 99, 99])
                .unwrap();
            frame
                .append(
                    &ImageTransfer {
                        bytes_written: 6,
                        ..strip(1)
                    },
                    &[7, 8, 9, 10, 11, 12],
                )
                .unwrap();
            assert_eq!(
                frame.finish().unwrap().into_rgb8().as_raw(),
                &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
            );
        }
    }
    #[test]
    fn refuses_duplicate_out_of_order_short_and_incomplete_strips() {
        let mut frame = Frame::new(info(2)).unwrap();
        assert!(frame.append(&strip(1), &[0; 8]).is_err());
        assert!(frame.append(&strip(0), &[0; 4]).is_err());
        frame.append(&strip(0), &[0; 8]).unwrap();
        assert!(frame.append(&strip(0), &[0; 8]).is_err());
        assert!(frame.finish().is_err());
        assert!(
            Frame::new(ImageInfo {
                width: 20000,
                height: 20000,
                ..info(2)
            })
            .is_err()
        );
        assert!(
            Frame::new(ImageInfo {
                planar: 1,
                ..info(2)
            })
            .is_err()
        );
    }
}
