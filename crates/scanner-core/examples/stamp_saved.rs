//! Read-only visual stamp comparison on checksum-verified prepared card fronts.
use scanner_types::scan::ScanRecord;
use serde_json::json;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: stamp_saved MANIFEST".into());
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
    let identities = record.identifications()?;
    let mut results = vec![];
    for source in &record.images {
        let Some(edit) = record
            .processed_images
            .as_deref()
            .unwrap_or_default()
            .iter()
            .rev()
            .find(|e| {
                e.request.sequence == source.sequence
                    && e.card_preparation.as_ref().is_some_and(|p| {
                        p.orientation_verified && p.orientation_basis.as_deref() == Some("text")
                    })
            })
        else {
            continue;
        };
        let bytes = root.read_image(&record, &edit.image, 32 * 1024 * 1024)?;
        let identity = identities.iter().rev().find(|i| {
            i.request.sequence == source.sequence
                && i.source_edit_id == Some(edit.request.id)
                && i.source_sha256 == edit.image.sha256
        });
        let orientation = identity
            .and_then(|i| i.evidence.as_ref())
            .map_or(0, |e| e.orientation);
        let result = scanner_recognition::finish::stamp::analyze(&bytes, orientation)?;
        eprintln!(
            "Stamp B-{:03} sequence {}: {:?}",
            record.batch_number.unwrap_or(0),
            source.sequence,
            result.state
        );
        results.push(
            json!({"sequence":source.sequence,"sourceSHA256":edit.image.sha256,"stamp":result,
            "savedEstimate":identity.and_then(|i|i.extensions.get("finishEstimate"))}),
        );
    }
    if std::fs::read(&manifest)? != original {
        return Err("Fixture manifest changed".into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"batch":record.batch_number,"results":results}))?
    );
    Ok(())
}
