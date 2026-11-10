use super::super::support::Harness;
use scanner_types::{
    identification::{CardIdentification, IdentificationRequest, IdentificationState},
    images::{CardPreparation, ImageCrop, ImageEditRequest},
    recognition::{CardCondition, CardFinish},
    scan::{ScanRecord, ScanRequest},
};
use uuid::Uuid;

pub(super) async fn saved_pair(
    mut h: Harness,
) -> (Harness, ScanRecord, CardIdentification, std::path::PathBuf) {
    let job = h
        .engine
        .start_scan(ScanRequest {
            duplex: true,
            ..Default::default()
        })
        .await
        .unwrap();
    h.next().await.finish.send(true).unwrap();
    h.terminal(job.id).await;
    while h.engine.status().await.active_job_id.is_some() {
        tokio::task::yield_now().await;
    }
    for sequence in 1..=2 {
        h.engine
            .edit_image(ImageEditRequest {
                id: Uuid::new_v4(),
                job_id: job.id,
                sequence,
                crop: ImageCrop::default(),
                quarter_turns: 0,
                deskew_degrees: None,
                extensions: Default::default(),
            })
            .await
            .unwrap();
    }
    let mut record = h.engine.scan_status(job.id).unwrap();
    for edit in record.processed_images.as_mut().unwrap() {
        edit.card_preparation = Some(CardPreparation {
            crop: ImageCrop::default(),
            quarter_turns: 0,
            orientation_verified: true,
            warnings: vec![],
            source_crop: None,
            deskew_degrees: None,
            orientation_basis: Some(
                if edit.request.sequence == 2 {
                    "pokemon-back-pattern"
                } else {
                    "text"
                }
                .into(),
            ),
            method: "test".into(),
            extensions: Default::default(),
        });
    }
    let source = &record.processed_images.as_ref().unwrap()[1];
    let path = std::path::Path::new(&record.output_directory).join(&source.image.relative_path);
    let mut identity = CardIdentification::pending(
        IdentificationRequest {
            id: Uuid::new_v4(),
            job_id: job.id,
            sequence: 2,
            language: Default::default(),
            extensions: Default::default(),
        },
        source.image.sha256.clone(),
        Some(source.request.id),
    );
    identity.state = IdentificationState::NoMatch;
    identity.finish = CardFinish::Holo;
    identity.condition = CardCondition::LightlyPlayed;
    identity
        .extensions
        .insert("futureEvidence".into(), serde_json::json!({"keep":true}));
    record.replace_identification(&identity).unwrap();
    h.engine.database().save(&record).unwrap();
    (h, record, identity, path)
}

#[tokio::test]
async fn checks_preserve_manual_assessments_require_current_identity_and_verify_exact_source() {
    let (h, record, identity, path) = saved_pair(Harness::new()).await;
    let checked = h
        .engine
        .check_condition(record.id, 2, identity.request.id)
        .await
        .unwrap();
    assert_eq!(
        checked.extensions["conditionCheck"]["state"],
        "inconclusive"
    ); // Fixture too small.
    let mut without_observation = serde_json::to_value(&checked).unwrap();
    without_observation
        .as_object_mut()
        .unwrap()
        .remove("conditionCheck");
    assert_eq!(
        without_observation,
        serde_json::to_value(&identity).unwrap()
    );
    let repeated = h
        .engine
        .check_condition(record.id, 2, identity.request.id)
        .await
        .unwrap();
    assert_eq!(repeated.extensions, checked.extensions);
    let manifest = h.engine.storage().read_manifest(&record).unwrap();
    assert_eq!(
        manifest.identifications().unwrap()[0].extensions,
        checked.extensions
    );
    assert_eq!(
        h.engine
            .check_condition(record.id, 2, Uuid::new_v4())
            .await
            .unwrap_err()
            .kind,
        "conflict"
    );
    std::fs::write(path, b"changed").unwrap();
    assert!(
        h.engine
            .check_condition(record.id, 2, identity.request.id)
            .await
            .is_err()
    );
    assert_eq!(
        h.engine
            .card_identification(record.id, 2)
            .await
            .unwrap()
            .unwrap()
            .extensions,
        checked.extensions
    );
    h.engine.shutdown().await;
}

struct FailedCatalogue;
#[async_trait::async_trait]
impl scanner_core::backend::CardRecognizer for FailedCatalogue {
    async fn recognize(
        &self,
        _: &ScanRecord,
        _: &scanner_types::images::OriginalImage,
    ) -> scanner_types::error::Result<scanner_types::recognition::RecognitionEvidence> {
        Err(scanner_types::error::ScannerError::new(
            "catalogue",
            "Fixture catalogue unavailable",
        ))
    }
}
#[tokio::test]
async fn automatic_condition_observations_survive_catalogue_failure() {
    let (h, record, identity, _) = saved_pair(Harness::with_recognizer(Some(std::sync::Arc::new(
        FailedCatalogue,
    ))))
    .await;
    let mut request = identity.request;
    request.id = Uuid::new_v4();
    h.engine.identify_card(request).await.unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let result = h
                .engine
                .card_identification(record.id, 2)
                .await
                .unwrap()
                .unwrap();
            if result.state != IdentificationState::Processing {
                break result;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(result.state, IdentificationState::Failed);
    assert_eq!(result.extensions["conditionCheck"]["state"], "inconclusive");
    assert_eq!(result.condition, CardCondition::Unassessed);
    h.engine.shutdown().await;
}
