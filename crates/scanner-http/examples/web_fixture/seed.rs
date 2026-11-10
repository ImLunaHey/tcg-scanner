use scanner_files::{BatchStore, StorageRoot};
use scanner_store::Database;
use scanner_types::{
    device::Device,
    identification::{CardIdentification, IdentificationRequest, IdentificationState},
    scan::ScanRequest,
};
use serde_json::json;
use std::error::Error;
pub fn seed(root: &StorageRoot, database: &Database) -> Result<(), Box<dyn Error>> {
    let mut batch = BatchStore::create(
        root,
        Device {
            id: "browser-fixture".into(),
            name: "Browser fixture".into(),
            ..Device::default()
        },
        ScanRequest {
            prepare_cards: Some(true),
            ..ScanRequest::default()
        },
        1,
    )?;
    let pixels = image::RgbaImage::from_fn(200, 300, |x, y| {
        image::Rgba([30 + (x / 2) as u8, 40 + (y / 3) as u8, 180, 255])
    });
    let image = scanner_images::encode_png(pixels)?;
    std::fs::write(batch.incoming_path().join("fixture.png"), image.bytes)?;
    let image = batch.preserve("fixture.png", 1)?;
    let mut identification = CardIdentification::pending(
        IdentificationRequest {
            id: uuid::Uuid::new_v4(),
            job_id: batch.record().id,
            sequence: 1,
            language: Default::default(),
            extensions: Default::default(),
        },
        image.sha256,
        None,
    );
    identification.state = IdentificationState::ReviewRequired;
    identification.evidence = Some(serde_json::from_value(
        json!({"lines":[],"warnings":["Synthetic browser fixture; not recognition evidence."],"orientation":0,"candidates":[{"id":"fixture-1","name":"Browser test card","localID":"001","setID":"fixture","setName":"Browser fixture","evidence":[]}]}),
    )?);
    let mut record = batch.record().clone();
    record.replace_identification(&identification)?;
    batch.update(|r| *r = record)?;
    batch.finish(vec![], false, true)?;
    database.insert(batch.record(), &batch.record().request)?;
    Ok(())
}
