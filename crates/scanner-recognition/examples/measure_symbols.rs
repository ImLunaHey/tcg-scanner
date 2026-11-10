//! Read-only saved-image benchmark for portable rarity and Energy measurements.
use scanner_recognition::{ocr::Tesseract, symbols};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize)]
struct Fixture {
    id: String,
    path: PathBuf,
    turns: u32,
    lines: Vec<scanner_types::recognition::RecognizedLine>,
    expected: Option<String>,
}
#[derive(Serialize)]
struct Measurement {
    id: String,
    expected: Option<String>,
    rarity: Option<scanner_types::printed::RaritySymbol>,
    energy: Option<scanner_types::printed::EnergySymbol>,
    words: Vec<scanner_types::recognition::RecognizedLine>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 4 {
        return Err("usage: measure_symbols FIXTURES TESSERACT_LIBRARY TESSDATA OUTPUT".into());
    }
    let fixtures: Vec<Fixture> = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    let mut ocr = Tesseract::new(&args[1], &args[2], "en")?;
    let mut measurements = vec![];
    for fixture in fixtures {
        let pixels = scanner_images::decode::decode(&std::fs::read(fixture.path)?)?.to_rgb8();
        let image = match fixture.turns {
            0 => pixels,
            1 => image::imageops::rotate90(&pixels),
            2 => image::imageops::rotate180(&pixels),
            3 => image::imageops::rotate270(&pixels),
            _ => return Err("Invalid quarter turns".into()),
        };
        let words = ocr.read_footer_words(&image)?;
        let rarity = symbols::rarity::analyze(&image, &words);
        let energy = symbols::energy::analyze(&image, &fixture.lines);
        eprintln!(
            "{} expected={:?} rarity={:?}",
            fixture.id,
            fixture.expected,
            rarity.as_ref().map(|r| &r.symbol)
        );
        measurements.push(Measurement {
            id: fixture.id,
            expected: fixture.expected,
            rarity,
            energy,
            words,
        });
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[3])?
        .write_all(&serde_json::to_vec_pretty(&measurements)?)?;
    Ok(())
}
use std::io::Write;
