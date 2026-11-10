//! Read-only diagnostics for portable set-code OCR in full-image coordinates.
use scanner_recognition::ocr::Tesseract;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("Usage: inspect_finish IMAGE LIBRARY DATA QUARTER_TURNS".into());
    }
    let pixels = scanner_images::decode::decode(&std::fs::read(&args[0])?)?.to_rgb8();
    let pixels = match args[3].parse::<u32>()? {
        0 => pixels,
        1 => image::imageops::rotate90(&pixels),
        2 => image::imageops::rotate180(&pixels),
        3 => image::imageops::rotate270(&pixels),
        _ => return Err("Quarter turns must be 0–3".into()),
    };
    let mut ocr = Tesseract::new(
        std::path::Path::new(&args[1]),
        std::path::Path::new(&args[2]),
        "en",
    )?;
    for magnify in [false, true] {
        println!(
            "{}",
            serde_json::to_string(
                &serde_json::json!({"magnify":magnify,"words":ocr.read_set_code(&pixels,magnify)?})
            )?
        );
    }
    Ok(())
}
