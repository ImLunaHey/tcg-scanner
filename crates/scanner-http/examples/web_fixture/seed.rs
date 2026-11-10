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
            duplex: true,
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
    let back = scanner_images::encode_png(image::RgbaImage::from_pixel(
        200,
        300,
        image::Rgba([40, 90, 150, 255]),
    ))?;
    std::fs::write(batch.incoming_path().join("fixture-back.png"), back.bytes)?;
    batch.preserve("fixture-back.png", 2)?;
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
    identification.evidence.as_mut().unwrap().extensions.insert("visualMatches".into(),json!([{
        "similarity":0.9,"candidate":{"id":"fixture-visual","name":"Browser visual card","localID":"002","setID":"fixture","setName":"Browser fixture","evidence":["Similar artwork; verify the printed set and number."]}
    }]));
    identification.extensions.insert("conditionCheck".into(),json!({"method":"synthetic-browser-fixture","state":"checked","limitations":["Synthetic mark; not measured wear."],"findings":[{"kind":"edgeWhitening","explanation":"Synthetic border mark for UI testing.","region":{"x":0.04,"y":0.4,"width":0.05,"height":0.06}}]}));
    let mut record = batch.record().clone();
    record.replace_identification(&identification)?;
    batch.update(|r| *r = record)?;
    batch.finish(vec![], false, true)?;
    database.insert(batch.record(), &batch.record().request)?;
    Ok(())
}
