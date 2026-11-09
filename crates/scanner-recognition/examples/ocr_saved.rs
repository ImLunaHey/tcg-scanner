//! Read local saved fixtures with native Tesseract; no platform OCR or upload.
use scanner_recognition::ocr::Tesseract;
use serde::Deserialize;
use serde_json::json;
use std::{path::PathBuf, time::Instant};
#[derive(Deserialize)]
struct Fixture {
    id: String,
    path: PathBuf,
    expected: Option<String>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 4 {
        return Err("usage: ocr_saved INPUT TESSERACT_LIBRARY TESSDATA OUTPUT".into());
    }
    let fixtures: Vec<Fixture> = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    let mut ocr = Tesseract::new(&args[1], &args[2], "en")?;
    let mut results = vec![];
    let start = Instant::now();
    for fixture in fixtures {
        let pixels = scanner_images::decode::decode(&std::fs::read(fixture.path)?)?.to_rgb8();
        let readings = ocr.read_card(&pixels)?;
        let regional = readings.regional();
        let combined: Vec<_> = readings.whole.iter().chain(&regional).cloned().collect();
        eprintln!("{}", fixture.id);
        results.push(
            json!({"id":fixture.id,"expected":fixture.expected,"readings":readings,
            "variants":{"whole":readings.whole,"regional":regional,"combined":combined}}),
        );
        std::fs::write(
            &args[3],
            serde_json::to_vec_pretty(
                &json!({"seconds":start.elapsed().as_secs_f64(),"results":results}),
            )?,
        )?;
    }
    Ok(())
}
