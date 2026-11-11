//! Diagnose camera extraction on a local image; optionally crop a screenshot's guide.
use std::{path::PathBuf, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !matches!(args.len(), 2 | 6) {
        return Err("camera_probe INPUT OUTPUT_DIRECTORY [X Y WIDTH HEIGHT]".into());
    }
    let mut image = image::open(&args[0])?;
    if args.len() == 6 {
        image = image.crop_imm(
            args[2].parse()?,
            args[3].parse()?,
            args[4].parse()?,
            args[5].parse()?,
        );
    }
    let image = image
        .resize_exact(1000, 1333, image::imageops::FilterType::Lanczos3)
        .to_rgb8();
    let output = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&output)?;
    let mut bytes = vec![];
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 82).encode_image(&image)?;
    std::fs::write(output.join("frame.jpg"), &bytes)?;
    let start = Instant::now();
    match scanner_images::camera::extract(&bytes)? {
        Some(card) => {
            std::fs::write(output.join("extracted.png"), &card.bytes)?;
            println!(
                "extracted {}x{} in {:?}",
                card.width,
                card.height,
                start.elapsed()
            );
        }
        None => println!("noCard in {:?}", start.elapsed()),
    }
    Ok(())
}
