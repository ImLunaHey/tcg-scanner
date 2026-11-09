use crate::storage;
use scanner_types::{error::Result, scan::OriginalFormat};
use std::io::{BufReader, Seek, SeekFrom};

pub(super) struct Metadata {
    pub width: u32,
    pub height: u32,
    pub count: u32,
    pub uniform_type: &'static str,
}

pub(super) fn inspect(file: &mut cap_std::fs::File, format: OriginalFormat) -> Result<Metadata> {
    file.seek(SeekFrom::Start(0)).map_err(storage)?;
    let reader = BufReader::new(file);
    let result = match format {
        OriginalFormat::Png => {
            let decoder = png::Decoder::new(reader).read_info().map_err(storage)?;
            let info = decoder.info();
            Metadata {
                width: info.width,
                height: info.height,
                count: info
                    .animation_control
                    .map(|control| control.num_frames)
                    .unwrap_or(1),
                uniform_type: "public.png",
            }
        }
        OriginalFormat::Tiff => {
            let mut decoder = tiff::decoder::Decoder::new(reader).map_err(storage)?;
            let (width, height) = decoder.dimensions().map_err(storage)?;
            let mut count = 1;
            while decoder.more_images() {
                if count >= 1000 {
                    return Err(storage("TIFF exceeds the page-count limit."));
                }
                decoder.next_image().map_err(storage)?;
                count += 1;
            }
            Metadata {
                width,
                height,
                count,
                uniform_type: "public.tiff",
            }
        }
    };
    if result.width == 0 || result.height == 0 || result.count == 0 || result.count > 1000 {
        return Err(storage("Invalid image dimensions or frame count."));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_tiff_frame_count_without_flattening_the_original() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pages.tiff");
        {
            let mut encoder =
                tiff::encoder::TiffEncoder::new(std::fs::File::create(&path).unwrap()).unwrap();
            encoder
                .write_image::<tiff::encoder::colortype::RGB8>(1, 1, &[1, 2, 3])
                .unwrap();
            encoder
                .write_image::<tiff::encoder::colortype::RGB8>(1, 1, &[4, 5, 6])
                .unwrap();
        }
        let mut file = cap_std::fs::File::from_std(std::fs::File::open(path).unwrap());
        let metadata = inspect(&mut file, OriginalFormat::Tiff).unwrap();
        assert_eq!(metadata.count, 2);
        assert!(inspect(&mut file, OriginalFormat::Png).is_err());
    }
}
