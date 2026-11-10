//! Read-only ordinary-finish evaluation, keeping identity fixed to isolate visual/OCR evidence.
use scanner_recognition::{finish, ocr::Tesseract};
use scanner_types::scan::ScanRecord;
use serde_json::json;
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !matches!(args.len(), 1 | 3) {
        return Err("Usage: finish_saved MANIFEST [TESSERACT_LIBRARY TRAINED_DATA]".into());
    }
    let manifest = std::fs::canonicalize(&args[0])?;
    let original = std::fs::read(&manifest)?;
    if original.len() > 16 * 1024 * 1024 {
        return Err("Manifest exceeds 16 MiB".into());
    }
    let mut record: ScanRecord = serde_json::from_slice(&original)?;
    let directory = manifest.parent().ok_or("Missing fixture directory")?;
    record.output_directory = directory.to_string_lossy().into();
    let root =
        scanner_files::StorageRoot::open(directory.parent().ok_or("Missing storage parent")?)?;
    let mut ocr = if args.len() == 3 {
        Some(Tesseract::new(
            Path::new(&args[1]),
            Path::new(&args[2]),
            "en",
        )?)
    } else {
        None
    };
    let results = evaluate(&root, &record, &mut ocr)?;
    if std::fs::read(&manifest)? != original {
        return Err("Fixture manifest changed".into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"batch":record.batch_number,"results":results}))?
    );
    Ok(())
}

fn evaluate(
    root: &scanner_files::StorageRoot,
    record: &ScanRecord,
    ocr: &mut Option<Tesseract>,
) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    let mut results = vec![];
    for identity in record.identifications()? {
        let Some(evidence) = identity
            .evidence
            .as_ref()
            .filter(|e| e.candidates.len() == 1)
        else {
            continue;
        };
        let edit = record
            .processed_images
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|e| {
                Some(e.request.id) == identity.source_edit_id
                    && e.request.sequence == identity.request.sequence
            });
        let image = edit
            .map(|e| &e.image)
            .or_else(|| {
                record
                    .images
                    .iter()
                    .find(|i| i.sequence == identity.request.sequence)
            })
            .ok_or("Missing source")?;
        if image.sha256 != identity.source_sha256 {
            return Err("Identification source changed".into());
        }
        let bytes = root.read_image(record, image, 32 * 1024 * 1024)?;
        let preparation = edit.and_then(|e| e.card_preparation.as_ref());
        let stored_text = finish::ordinary(
            &bytes,
            evidence,
            preparation,
            identity.request.language,
            None,
        );
        let portable_text = ocr.as_mut().map(|ocr| {
            let mut without_vision = evidence.clone();
            without_vision.lines.clear();
            finish::ordinary(
                &bytes,
                &without_vision,
                preparation,
                identity.request.language,
                Some(ocr),
            )
        });
        eprintln!(
            "Ordinary finish B-{:03} sequence {}",
            record.batch_number.unwrap_or(0),
            identity.request.sequence
        );
        results.push(json!({"sequence":identity.request.sequence,"cardID":evidence.candidates[0].id,
            "savedEstimate":identity.extensions.get("finishEstimate"),"manualFinish":identity.finish,
            "storedText":stored_text,"portableText":portable_text}));
    }
    Ok(results)
}
