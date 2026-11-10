//! Read-only comparison of native observations on checksum-verified saved cards.
use scanner_types::scan::ScanRecord;
use serde_json::json;
use std::{collections::BTreeMap, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 1 {
        return Err("Usage: condition_saved MANIFEST".into());
    }
    let manifest = std::fs::canonicalize(&arguments[0])?;
    let original = std::fs::read(&manifest)?;
    if original.len() > 16 * 1024 * 1024 {
        return Err("Manifest exceeds 16 MiB".into());
    }
    let mut record: ScanRecord = serde_json::from_slice(&original)?;
    let directory = manifest.parent().ok_or("No fixture directory")?;
    record.output_directory = directory.to_string_lossy().into();
    let root = scanner_files::StorageRoot::open(directory.parent().ok_or("No storage parent")?)?;
    let mut states = BTreeMap::<String, usize>::new();
    let mut results = vec![];
    for source in &record.images {
        let Some(edit) = record
            .processed_images
            .as_deref()
            .unwrap_or_default()
            .iter()
            .rev()
            .find(|e| e.request.sequence == source.sequence && e.card_preparation.is_some())
        else {
            continue;
        };
        let bytes = root.read_image(&record, &edit.image, 32 * 1024 * 1024)?;
        let result = scanner_recognition::condition::check(&bytes, edit.card_preparation.as_ref());
        let mut unknown = edit.card_preparation.clone().unwrap();
        unknown.orientation_verified = false;
        unknown.orientation_basis = Some("unknown".into());
        let fallback = scanner_recognition::condition::check(&bytes, Some(&unknown));
        let state = serde_json::to_value(result.state)?
            .as_str()
            .unwrap()
            .to_owned();
        *states.entry(state).or_default() += 1;
        results.push(json!({"sequence":source.sequence,"basis":edit.card_preparation.as_ref().and_then(|p| p.orientation_basis.as_deref()),
            "condition":result,"fallbackState":fallback.state,"fallbackFindings":fallback.findings.len()}));
    }
    if std::fs::read(Path::new(&manifest))? != original {
        return Err("Fixture manifest changed".into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"batch":record.batch_number,"states":states,"results":results})
        )?
    );
    Ok(())
}
