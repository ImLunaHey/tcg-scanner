//! Local development probe for printed-field OCR. Never changes its source.
use scanner_recognition::ocr::Tesseract;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 8 {
        return Err("usage: inspect_region INPUT LIBRARY DATA OUTPUT_PNG X Y WIDTH HEIGHT".into());
    }
    let pixels = scanner_images::decode::decode(&std::fs::read(&args[0])?)?.to_rgb8();
    let region: Vec<f64> = args[4..]
        .iter()
        .map(|s| s.parse())
        .collect::<Result<_, _>>()?;
    if region.iter().any(|n| !n.is_finite() || *n < 0.)
        || region[2] == 0.
        || region[3] == 0.
        || region[0] + region[2] > 1.
        || region[1] + region[3] > 1.
    {
        return Err("Invalid normalized region".into());
    }
    let crop = image::imageops::crop_imm(
        &pixels,
        (region[0] * f64::from(pixels.width())) as u32,
        (region[1] * f64::from(pixels.height())) as u32,
        (region[2] * f64::from(pixels.width())).max(1.) as u32,
        (region[3] * f64::from(pixels.height())).max(1.) as u32,
    )
    .to_image();
    let mut encoded = std::io::Cursor::new(Vec::new());
    crop.write_to(&mut encoded, image::ImageFormat::Png)?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[3])?;
    std::io::Write::write_all(&mut output, &encoded.into_inner())?;
    let mut ocr = Tesseract::new(
        std::path::Path::new(&args[1]),
        std::path::Path::new(&args[2]),
        "en",
    )?;
    let mut padded =
        image::RgbImage::from_pixel(crop.width() + 32, crop.height() + 32, image::Rgb([255; 3]));
    image::imageops::replace(&mut padded, &crop, 16, 16);
    for threshold in [0u8, 80, 100, 120, 140, 160, 180, 200] {
        let mut pixels = image::imageops::resize(
            &padded,
            padded.width() * 4,
            padded.height() * 4,
            image::imageops::FilterType::CatmullRom,
        );
        if threshold > 0 {
            for pixel in pixels.pixels_mut() {
                let [r, g, b] = pixel.0.map(f32::from);
                let value = if r * 0.2126 + g * 0.7152 + b * 0.0722 < f32::from(threshold) {
                    0
                } else {
                    255
                };
                *pixel = image::Rgb([value; 3]);
            }
        }
        println!(
            "{}",
            serde_json::to_string(
                &serde_json::json!({"threshold":threshold,"lines":ocr.recognize(&pixels,true)?})
            )?
        );
    }
    Ok(())
}
