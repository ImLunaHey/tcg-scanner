use super::ffi::{self, Parameters};
use image::{DynamicImage, RgbImage};
use scanner_types::error::Result;

const MAX_PIXELS: usize = 48_000_000;
const MAX_BYTES: usize = 160 * 1024 * 1024;
pub(super) struct Frame {
    parameters: Parameters,
    bytes: Vec<u8>,
    limit: usize,
}
impl Frame {
    pub fn new(parameters: Parameters) -> Result<Self> {
        let p = parameters;
        if p.format != 1 || p.depth != 8 || p.last_frame != 1 {
            return Err(ffi::error(
                "SANE must deliver a complete interleaved 8-bit RGB frame.",
            ));
        }
        if !(1..=20000).contains(&p.pixels_per_line)
            || !(1..=20000).contains(&p.lines) && p.lines != -1
        {
            return Err(ffi::error("Invalid SANE image dimensions."));
        }
        let width = p.pixels_per_line as usize;
        if p.bytes_per_line < p.pixels_per_line * 3
            || p.bytes_per_line > p.pixels_per_line * 3 + 65536
        {
            return Err(ffi::error("Invalid SANE row stride."));
        }
        let max_lines = (MAX_PIXELS / width).min(20000);
        let lines = if p.lines == -1 {
            max_lines
        } else {
            p.lines as usize
        };
        let limit = (p.bytes_per_line as usize)
            .checked_mul(lines)
            .ok_or_else(|| ffi::error("SANE image size overflow."))?;
        if lines > max_lines || limit > MAX_BYTES {
            return Err(ffi::error("SANE image exceeds capture limits."));
        }
        Ok(Self {
            parameters,
            bytes: Vec::new(),
            limit,
        })
    }
    pub fn push(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > self.limit - self.bytes.len() {
            return Err(ffi::error(
                "SANE frame exceeds its declared or maximum length.",
            ));
        }
        self.bytes.try_reserve(bytes.len()).map_err(ffi::error)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    pub fn finish(self) -> Result<DynamicImage> {
        let stride = self.parameters.bytes_per_line as usize;
        let width = self.parameters.pixels_per_line as usize;
        if self.bytes.is_empty()
            || !self.bytes.len().is_multiple_of(stride)
            || self.parameters.lines != -1 && self.bytes.len() != self.limit
        {
            return Err(ffi::error("SANE returned an empty or truncated frame."));
        }
        let height = self.bytes.len() / stride;
        let mut rgb = Vec::with_capacity(width * height * 3);
        for row in self.bytes.chunks_exact(stride) {
            rgb.extend_from_slice(&row[..width * 3]);
        }
        Ok(DynamicImage::ImageRgb8(
            RgbImage::from_raw(width as u32, height as u32, rgb)
                .ok_or_else(|| ffi::error("Invalid RGB frame."))?,
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn parameters(lines: i32) -> Parameters {
        Parameters {
            format: 1,
            last_frame: 1,
            bytes_per_line: 8,
            pixels_per_line: 2,
            lines,
            depth: 8,
        }
    }
    #[test]
    fn strips_padding_and_accepts_unknown_height_across_partial_reads() {
        let mut frame = Frame::new(parameters(-1)).unwrap();
        frame.push(&[1, 2, 3]).unwrap();
        frame
            .push(&[4, 5, 6, 99, 99, 7, 8, 9, 10, 11, 12, 99, 99])
            .unwrap();
        let image = frame.finish().unwrap().into_rgb8();
        assert_eq!(image.dimensions(), (2, 2));
        assert_eq!(image.as_raw(), &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    }
    #[test]
    fn rejects_truncated_oversized_and_unsupported_frames() {
        let mut frame = Frame::new(parameters(2)).unwrap();
        frame.push(&[0; 8]).unwrap();
        assert!(frame.finish().is_err());
        let mut frame = Frame::new(parameters(1)).unwrap();
        assert!(frame.push(&[0; 9]).is_err());
        assert!(
            Frame::new(Parameters {
                depth: 16,
                ..parameters(1)
            })
            .is_err()
        );
        assert!(
            Frame::new(Parameters {
                last_frame: 0,
                ..parameters(1)
            })
            .is_err()
        );
        assert!(
            Frame::new(Parameters {
                pixels_per_line: 20000,
                bytes_per_line: 60000,
                lines: 20000,
                ..parameters(1)
            })
            .is_err()
        );
        let mut frame = Frame::new(parameters(-1)).unwrap();
        frame.push(&[0; 7]).unwrap();
        assert!(frame.finish().is_err());
    }
}
