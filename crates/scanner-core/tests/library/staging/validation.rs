use super::fixture::{prepared, settled};
use scanner_types::{
    edition::{EditionEvidence, EditionState, StampEvidence, StampState},
    finish::{FinishEstimate, FinishState},
    staging::TransferState,
};
use std::sync::atomic::Ordering;

#[tokio::test]
async fn preparation_and_current_stamp_evidence_block_unsafe_new_sends_and_frozen_retries() {
    let (h, record, store) = prepared().await;
    for change in ["orientation", "source", "language", "finish"] {
        let mut invalid = record.clone();
        match change {
            "orientation" => {
                invalid
                    .processed_images
                    .as_mut()
                    .unwrap()
                    .last_mut()
                    .unwrap()
                    .card_preparation
                    .as_mut()
                    .unwrap()
                    .orientation_verified = false
            }
            "source" => {
                invalid
                    .processed_images
                    .as_mut()
                    .unwrap()
                    .last_mut()
                    .unwrap()
                    .source_sha256 = "changed".into()
            }
            "language" => {
                invalid.request.card_language = Some(scanner_types::cards::CardLanguage::Korean)
            }
            "finish" => {
                let mut identity = invalid.identifications().unwrap().remove(0);
                identity
                    .extensions
                    .insert("finishEstimate".into(), serde_json::json!("unreadable"));
                invalid.replace_identification(&identity).unwrap();
            }
            _ => unreachable!(),
        }
        h.engine.database().save(&invalid).unwrap();
        assert!(h.engine.send_to_store(record.id).await.is_err(), "{change}");
    }
    assert_eq!(store.imports.load(Ordering::SeqCst), 0);
    h.engine.database().save(&record).unwrap();
    store.lose_upload.store(true, Ordering::SeqCst);
    h.engine.send_to_store(record.id).await.unwrap();
    assert_eq!(settled(&h, record.id).await.state, TransferState::Failed);
    let mut stamped = record;
    let mut identity = stamped.identifications().unwrap().remove(0);
    identity.extensions.insert(
        "finishEstimate".into(),
        serde_json::to_value(stamp()).unwrap(),
    );
    stamped.replace_identification(&identity).unwrap();
    h.engine.database().save(&stamped).unwrap();
    assert_eq!(
        h.engine.send_to_store(stamped.id).await.unwrap_err().kind,
        "storeNotReady"
    );
    assert_eq!(store.imports.load(Ordering::SeqCst), 1);
    h.engine.shutdown().await;
}
fn stamp() -> FinishEstimate {
    let mut finish = FinishEstimate::new(FinishState::Uncertain, None, vec![], None);
    finish.edition = Some(EditionEvidence::new(
        EditionState::Uncertain,
        None,
        StampEvidence {
            state: StampState::Uncertain,
            kind: "pikachu-pumpkin".into(),
            method: "fixture".into(),
            region: None,
            source_quarter_turns: 0,
            similarity: None,
            baseline_similarity: None,
            downsampled_similarity: None,
            evidence: vec![],
        },
        None,
        vec![],
        vec![],
    ));
    finish
}
#[tokio::test]
async fn corrupt_prepared_bytes_never_upload_and_recovery_retains_frozen_payload() {
    let (h, record, store) = prepared().await;
    let image = &record
        .processed_images
        .as_ref()
        .unwrap()
        .last()
        .unwrap()
        .image;
    std::fs::write(
        std::path::Path::new(&record.output_directory).join(&image.relative_path),
        b"changed",
    )
    .unwrap();
    h.engine.send_to_store(record.id).await.unwrap();
    assert_eq!(settled(&h, record.id).await.state, TransferState::Failed);
    assert_eq!(store.uploads.load(Ordering::SeqCst), 0);
    h.engine.shutdown().await;
    let db = h.engine.database();
    let mut receipt = db.store_transfer(record.id).unwrap().unwrap();
    let frozen = receipt["submission"].clone();
    receipt["status"]["state"] = serde_json::json!("sending");
    db.save_store_transfer(record.id, "sending", &receipt, &receipt["status"])
        .unwrap();
    scanner_core::recover_before_start(db, None).unwrap();
    let recovered = db.store_transfer(record.id).unwrap().unwrap();
    assert_eq!(recovered["status"]["state"], "failed");
    assert_eq!(recovered["status"]["error"]["kind"], "storeInterrupted");
    assert_eq!(recovered["submission"], frozen);
}
