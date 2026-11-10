//! Exercise the complete camera recognizer on a private local JPEG, without saving a scan.
use scanner_recognition::camera::CameraRecognizer;
use scanner_types::cards::CardLanguage;
use std::{path::PathBuf, time::Instant};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 3 {
        return Err("camera_saved JPEG TESSERACT_LIBRARY TESSDATA".into());
    }
    let recognizer = CameraRecognizer::new(args[1].clone(), args[2].clone())?;
    let started = Instant::now();
    let result = recognizer
        .recognize(std::fs::read(&args[0])?, CardLanguage::English)
        .await?;
    eprintln!("camera lookup: {:?}", started.elapsed());
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
