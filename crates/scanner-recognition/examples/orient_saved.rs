//! Verify native orientation against saved upright cards and all quarter turns.
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
        return Err("usage: orient_saved INPUT TESSERACT_LIBRARY TESSDATA OUTPUT".into());
    }
    let fixtures: Vec<Fixture> = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    let mut engine = Tesseract::new(&args[1], &args[2], "en")?;
    let mut results = vec![];
    let start = Instant::now();
    for fixture in fixtures {
        let pixels = scanner_images::decode::decode(&std::fs::read(fixture.path)?)?.to_rgb8();
        for turns in 0..4u8 {
            let rotated = match turns {
                1 => image::imageops::rotate90(&pixels),
                2 => image::imageops::rotate180(&pixels),
                3 => image::imageops::rotate270(&pixels),
                _ => pixels.clone(),
            };
            let result = engine.orientation(&rotated)?;
            eprintln!("{} {turns}: {:?}", fixture.id, result);
            results.push(json!({"id":fixture.id,"inputTurns":turns,"expectedTurns":(4-turns)%4,"back":fixture.expected.is_none(),"orientation":result}));
            std::fs::write(
                &args[3],
                serde_json::to_vec_pretty(
                    &json!({"seconds":start.elapsed().as_secs_f64(),"results":results}),
                )?,
            )?;
        }
    }
    Ok(())
}
