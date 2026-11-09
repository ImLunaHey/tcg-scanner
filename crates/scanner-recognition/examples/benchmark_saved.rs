//! Run the pinned Rust engines over the same local fixtures as the earlier JS experiment.
use scanner_recognition::{
    embedding::{DinoEncoder, EmbeddingIndex, ReferenceEmbedding},
    ocr::Tesseract,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Deserialize)]
struct Fixture {
    id: String,
    path: PathBuf,
    expected: Option<String>,
}
#[derive(Deserialize)]
struct Reference {
    id: String,
    path: PathBuf,
    sha256: String,
}
#[derive(Deserialize)]
struct References {
    references: Vec<Reference>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 7 {
        return Err("usage: benchmark_saved INPUT REFERENCES MODEL ORT_LIBRARY TESSERACT_LIBRARY TESSDATA OUTPUT".into());
    }
    let fixtures: Vec<Fixture> = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    let references: References = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let mut encoder = DinoEncoder::load(&args[3], &args[2])?;
    let mut ocr = Tesseract::new(&args[4], &args[5], "en")?;
    let model = encoder.model_key();
    let index = build_index(&mut encoder, &references)?;
    let mut results = vec![];
    let started = Instant::now();
    for fixture in &fixtures {
        let bytes = std::fs::read(&fixture.path)?;
        let pixels = scanner_images::decode::decode(&bytes)?.to_rgb8();
        let encoded = encoder.encode(&pixels)?;
        let ranked = index.search(model, &encoded, 5)?;
        let lines = ocr.recognize(&pixels, false)?;
        eprintln!("{}: {}", fixture.id, ranked[0].id);
        results.push(
            json!({"id":fixture.id,"expected":fixture.expected,"ranked":ranked,"lines":lines}),
        );
        save(
            &args[6],
            &results,
            &references,
            started.elapsed().as_secs_f64(),
            model,
        )?;
    }
    Ok(())
}

fn build_index(
    encoder: &mut DinoEncoder,
    references: &References,
) -> Result<EmbeddingIndex, Box<dyn std::error::Error>> {
    let mut vectors = vec![];
    for reference in &references.references {
        let bytes = std::fs::read(&reference.path)?;
        if format!("{:x}", Sha256::digest(&bytes)) != reference.sha256 {
            return Err("Reference checksum mismatch".into());
        }
        let image = scanner_images::decode::decode(&bytes)?.to_rgb8();
        vectors.push(ReferenceEmbedding {
            id: reference.id.clone(),
            sha256: reference.sha256.clone(),
            vector: encoder.encode(&image)?,
        });
    }
    Ok(EmbeddingIndex::new(encoder.model_key().into(), vectors)?)
}

fn save(
    path: &Path,
    results: &[Value],
    references: &References,
    seconds: f64,
    model: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let covered: Vec<_> = results
        .iter()
        .filter(|r| {
            r["expected"].as_str().is_some_and(|expected| {
                references
                    .references
                    .iter()
                    .any(|reference| reference.id == expected)
            })
        })
        .collect();
    let correct = covered
        .iter()
        .filter(|r| r["ranked"][0]["id"] == r["expected"])
        .count();
    let report = json!({"model":model,"referenceCount":references.references.len(),"seconds":seconds,
        "completed":results.len(),"covered":covered.len(),"top1Correct":correct,"results":results});
    std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}
