//! Development benchmark. Saved scans are padded into synthetic camera scenes;
//! cameraFrame inputs are already JPEG camera frames. No saved scans are mutated.
use scanner_recognition::{embedding::VisualRetriever, ocr::Tesseract};
use scanner_types::cards::CardLanguage;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    id: String,
    path: PathBuf,
    expected: Option<String>,
    #[serde(default)]
    camera_frame: bool,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 3 {
        return Err("camera_components INPUT SERVICE_CONFIG OUTPUT_DIRECTORY".into());
    }
    let fixtures: Vec<Fixture> = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    let config: Value = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let path = |key: &str| Path::new(config[key].as_str().unwrap());
    let visual = &config["visualRecognition"];
    let started = Instant::now();
    let retriever = VisualRetriever::load(
        Path::new(visual["runtimeLibrary"].as_str().unwrap()),
        Path::new(visual["model"].as_str().unwrap()),
        Path::new(visual["index"].as_str().unwrap()),
    )?;
    let model_load_ms = started.elapsed().as_secs_f64() * 1000.;
    std::fs::create_dir_all(&args[2])?;
    let mut rows = vec![];
    for fixture in fixtures {
        let bytes = frame(&fixture)?;
        let frame_path = args[2].join(format!("{}.jpg", fixture.id));
        std::fs::write(&frame_path, &bytes)?;
        for round in 0..3 {
            let start = Instant::now();
            let card = scanner_images::camera::extract(&bytes)?;
            let extract_ms = start.elapsed().as_secs_f64() * 1000.;
            let mut row = json!({"id":fixture.id,"expected":fixture.expected,"path":frame_path,
                "round":round,"synthetic":!fixture.camera_frame,"extractMs":extract_ms});
            if let Some(card) = card {
                let pixels = scanner_images::decode::decode(&card.bytes)?.to_rgb8();
                let start = Instant::now();
                let mut ocr = Tesseract::new(path("tesseractLibrary"), path("trainedData"), "en")?;
                ocr.set_deadline(Instant::now() + Duration::from_secs(60));
                let orientation = ocr.orientation(&pixels)?;
                row["orientationMs"] = json!(start.elapsed().as_secs_f64() * 1000.);
                row["orientationVerified"] = json!(orientation.verified);
                let start = Instant::now();
                let candidates = retriever
                    .search(
                        card.bytes,
                        u32::from(orientation.quarter_turns),
                        CardLanguage::English,
                    )
                    .await?;
                row["visualMs"] = json!(start.elapsed().as_secs_f64() * 1000.);
                row["visualCandidates"] = json!(candidates);
            }
            eprintln!("{} round {}", fixture.id, round);
            rows.push(row);
            std::fs::write(
                args[2].join("components.json"),
                serde_json::to_vec_pretty(&json!({"modelLoadMs":model_load_ms,"rows":rows}))?,
            )?;
        }
    }
    Ok(())
}
fn frame(fixture: &Fixture) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if fixture.camera_frame {
        return Ok(std::fs::read(&fixture.path)?);
    }
    let source = scanner_images::decode::decode(&std::fs::read(&fixture.path)?)?;
    let card = source.thumbnail(820, 1120).to_rgb8();
    let mut frame = image::RgbImage::from_pixel(1000, 1333, image::Rgb([35, 35, 35]));
    image::imageops::overlay(
        &mut frame,
        &card,
        i64::from((1000 - card.width()) / 2),
        i64::from((1333 - card.height()) / 2),
    );
    let mut bytes = vec![];
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 82).encode_image(&frame)?;
    Ok(bytes)
}
