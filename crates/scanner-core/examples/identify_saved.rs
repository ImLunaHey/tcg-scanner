use scanner_core::{NativeCardRecognizer, backend::CardRecognizer};
use scanner_files::StorageRoot;
use scanner_types::scan::ScanRecord;
use std::{path::PathBuf, sync::Arc};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err(
            "usage: identify_saved MANIFEST SEQUENCE TESSERACT_LIBRARY TRAINED_DATA".into(),
        );
    }
    let record: ScanRecord = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    let sequence: u32 = args[1].parse()?;
    let root = Arc::new(StorageRoot::open(
        std::path::Path::new(&record.output_directory)
            .parent()
            .ok_or("No storage parent")?,
    )?);
    let source = record
        .processed_images
        .as_deref()
        .unwrap_or_default()
        .iter()
        .rev()
        .find(|e| e.request.sequence == sequence && e.card_preparation.is_some())
        .map(|e| &e.image)
        .or_else(|| record.images.iter().find(|i| i.sequence == sequence))
        .ok_or("Image missing")?;
    let recognizer =
        NativeCardRecognizer::new(root, PathBuf::from(&args[2]), PathBuf::from(&args[3]))?;
    let evidence = recognizer.recognize(&record, source).await?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}
