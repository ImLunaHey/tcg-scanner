//! Prepare one saved original in an isolated batch; never write to the source share.
use scanner_core::{NativeCardProcessor, backend::CardProcessor};
use scanner_files::{BatchStore, StorageRoot};
use scanner_types::{
    device::Device,
    scan::{ScanRecord, ScanRequest},
};
use std::{path::PathBuf, sync::Arc};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 5 {
        return Err("usage: prepare_saved MANIFEST SEQUENCE TESSERACT_LIBRARY TESSDATA NEW_OUTPUT_DIRECTORY".into());
    }
    let sequence = args[1].to_str().ok_or("Invalid sequence")?.parse::<u32>()?;
    let saved: ScanRecord = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    let source = saved
        .images
        .iter()
        .find(|i| i.sequence == sequence)
        .ok_or("Missing sequence")?;
    let source_directory = PathBuf::from(&saved.output_directory);
    let sources = StorageRoot::open(source_directory.parent().ok_or("Missing source root")?)?;
    let bytes = sources.read_image(&saved, source, 32 * 1024 * 1024)?;
    std::fs::create_dir(&args[4])?;
    let root = Arc::new(StorageRoot::open(&args[4])?);
    let request = ScanRequest {
        format: saved.request.format,
        background: saved.request.background,
        card_language: saved.request.card_language,
        ..ScanRequest::default()
    };
    let filename = format!("fixture.{}", request.format.extension());
    let mut store = BatchStore::create(&root, Device::default(), request, 1)?;
    std::fs::write(store.incoming_path().join(&filename), &bytes)?;
    store.preserve(&filename, 1)?;
    let processor = NativeCardProcessor::new(root.clone(), args[2].clone(), args[3].clone());
    let edit = processor.prepare(store.record(), 1).await?;
    store.update(|record| record.processed_images = Some(vec![edit.clone()]))?;
    store.finish(vec![], false, true)?;
    if sources.read_image(&saved, source, 32 * 1024 * 1024)? != bytes {
        return Err("Source changed".into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"directory":store.record().output_directory,
        "sourceSHA256":source.sha256,"prepared":edit,"originalUnchanged":true})
        )?
    );
    Ok(())
}
