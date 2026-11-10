//! Read-only regression comparison with saved preparation geometry, never a manifest rewrite.
use scanner_images::boundary::prepare_geometry;
use scanner_types::{
    images::ImageCrop,
    scan::{ScanBackground, ScanRecord},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: compare_geometry MANIFEST")?;
    let record: ScanRecord = serde_json::from_slice(&std::fs::read(path)?)?;
    let root = Path::new(&record.output_directory).canonicalize()?;
    let mut seen = BTreeSet::new();
    let mut rows = vec![];
    let started = Instant::now();
    for previous in record.processed_images.as_ref().into_iter().flatten() {
        let Some(plan) = &previous.card_preparation else {
            continue;
        };
        if !seen.insert(previous.request.sequence) {
            continue;
        }
        let Some(source) = record
            .images
            .iter()
            .find(|image| image.sequence == previous.request.sequence)
        else {
            continue;
        };
        let path = root.join(&source.relative_path).canonicalize()?;
        if !path.starts_with(&root) {
            return Err("image escaped batch directory".into());
        }
        let bytes = std::fs::read(path)?;
        if format!("{:x}", Sha256::digest(&bytes)) != source.sha256 {
            return Err("source checksum mismatch".into());
        }
        let result = prepare_geometry(
            &bytes,
            record.request.background.unwrap_or(ScanBackground::Black),
        );
        rows.push(match result {
            Ok(new) => json!({"sequence":source.sequence,"sourceCropIoU":overlap(new.source_crop.unwrap(),plan.source_crop.unwrap_or(plan.crop)),
                "previousCrop":plan.source_crop.unwrap_or(plan.crop),"rustCrop":new.source_crop,
                "previousDegrees":plan.deskew_degrees.unwrap_or(0.),"rustDegrees":new.deskew_degrees.unwrap_or(0.),
                "angleDifference":(new.deskew_degrees.unwrap_or(0.)-plan.deskew_degrees.unwrap_or(0.)).abs(),"warnings":new.warnings}),
            Err(error) => json!({"sequence":source.sequence,"error":error}),
        });
    }
    let successes: Vec<_> = rows.iter().filter(|r| r["error"].is_null()).collect();
    let min_iou = successes
        .iter()
        .filter_map(|r| r["sourceCropIoU"].as_f64())
        .reduce(f64::min);
    let max_angle = successes
        .iter()
        .filter_map(|r| r["angleDifference"].as_f64())
        .reduce(f64::max);
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"compared":rows.len(),"succeeded":successes.len(),
        "minimumSourceCropIoU":min_iou,"maximumAngleDifference":max_angle,"seconds":started.elapsed().as_secs_f64(),"rows":rows}))?
    );
    Ok(())
}

fn overlap(a: ImageCrop, b: ImageCrop) -> f64 {
    let width = ((a.x + a.width).min(b.x + b.width) - a.x.max(b.x)).max(0.);
    let height = ((a.y + a.height).min(b.y + b.height) - a.y.max(b.y)).max(0.);
    let intersection = width * height;
    intersection / (a.width * a.height + b.width * b.height - intersection)
}
