use scanner_core::{NativeCardRecognizer, backend::CardRecognizer};
use scanner_files::StorageRoot;
use scanner_types::scan::ScanRecord;
use std::{path::PathBuf, sync::Arc};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(4..=5).contains(&args.len()) {
        return Err("usage: identify_saved MANIFEST SEQUENCE|all TESSERACT_LIBRARY TRAINED_DATA [CACHE_SQLITE]".into());
    }
    let manifest = std::fs::canonicalize(&args[0])?;
    let bytes = std::fs::read(&manifest)?;
    let mut record: ScanRecord = serde_json::from_slice(&bytes)?;
    // Archived manifests retain their original absolute directory. Rebase only
    // this read-only in-memory copy to the explicitly supplied fixture location.
    let directory = manifest.parent().ok_or("No fixture directory")?;
    record.output_directory = directory.to_string_lossy().into();
    let root = Arc::new(StorageRoot::open(
        directory.parent().ok_or("No storage parent")?,
    )?);
    let mut recognizer =
        NativeCardRecognizer::new(root, PathBuf::from(&args[2]), PathBuf::from(&args[3]))?;
    if let Some(cache) = args.get(4) {
        recognizer = recognizer.with_documents(scanner_catalogue::persistent::Documents::open(
            std::path::Path::new(cache),
        )?);
    }
    let identities = record.identifications()?;
    let sequences: Vec<u32> = if args[1] == "all" {
        identities
            .iter()
            .filter(|i| {
                i.evidence
                    .as_ref()
                    .is_some_and(|e| !e.candidates.is_empty())
            })
            .map(|i| i.request.sequence)
            .collect()
    } else {
        vec![args[1].parse()?]
    };
    let mut results = vec![];
    for sequence in sequences {
        let source = source(&record, sequence)?;
        let evidence = recognizer.recognize(&record, source).await?;
        let expected = identities
            .iter()
            .find(|i| i.request.sequence == sequence)
            .and_then(|i| {
                i.selected_card_id.clone().or_else(|| {
                    i.evidence
                        .as_ref()
                        .and_then(|e| (e.candidates.len() == 1).then(|| e.candidates[0].id.clone()))
                })
            });
        eprintln!(
            "Sequence {sequence}: {} candidate(s)",
            evidence.candidates.len()
        );
        results
            .push(serde_json::json!({"sequence":sequence,"expected":expected,"evidence":evidence}));
    }
    assert_eq!(
        std::fs::read(manifest)?,
        bytes,
        "Fixture manifest must remain unchanged"
    );
    if args[1] == "all" {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"results":results}))?
        );
    } else {
        println!("{}", serde_json::to_string_pretty(&results[0]["evidence"])?);
    }
    Ok(())
}

fn source(
    record: &ScanRecord,
    sequence: u32,
) -> Result<&scanner_types::images::OriginalImage, Box<dyn std::error::Error>> {
    record
        .processed_images
        .as_deref()
        .unwrap_or_default()
        .iter()
        .rev()
        .find(|e| e.request.sequence == sequence && e.card_preparation.is_some())
        .map(|e| &e.image)
        .or_else(|| record.images.iter().find(|i| i.sequence == sequence))
        .ok_or_else(|| "Image missing".into())
}
