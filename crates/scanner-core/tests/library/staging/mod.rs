mod fixture;
mod validation;
use fixture::{prepared, settled};
use scanner_types::{cards::CardLanguage, staging::TransferState};
use std::sync::atomic::Ordering;

#[tokio::test]
async fn frozen_retry_recovers_lost_upload_without_reupload_and_resync_preserves_reviewers() {
    let (h, mut record, store) = prepared().await;
    store.lose_upload.store(true, Ordering::SeqCst);
    assert_eq!(
        h.engine.send_to_store(record.id).await.unwrap().state,
        TransferState::Sending
    );
    assert_eq!(settled(&h, record.id).await.state, TransferState::Failed);
    assert_eq!(store.uploads.load(Ordering::SeqCst), 1);
    h.engine.send_to_store(record.id).await.unwrap();
    let sent = settled(&h, record.id).await;
    assert_eq!(sent.state, TransferState::Sent);
    assert_eq!(sent.sent_cards, 1);
    assert_eq!(store.uploads.load(Ordering::SeqCst), 1);
    assert_eq!(
        sent.review_url.as_deref(),
        Some("https://store.example/review/fixture")
    );
    record.request.card_language = Some(CardLanguage::Korean);
    for mut identity in record.identifications().unwrap() {
        identity.request.language = CardLanguage::Korean;
        record.replace_identification(&identity).unwrap();
    }
    h.engine.database().save(&record).unwrap();
    h.engine.send_to_store(record.id).await.unwrap();
    assert_eq!(settled(&h, record.id).await.sent_cards, 1);
    assert_eq!(store.updates.load(Ordering::SeqCst), 1);
    store.remote.lock().unwrap().as_mut().unwrap().notes = Some("Reviewer note".into());
    h.engine.send_to_store(record.id).await.unwrap();
    let preserved = settled(&h, record.id).await;
    assert_eq!(preserved.state, TransferState::Sent);
    assert_eq!(preserved.sent_cards, 0);
    assert_eq!(preserved.preserved_cards.unwrap(), ["C-001"]);
    assert_eq!(store.updates.load(Ordering::SeqCst), 1);
    assert_eq!(store.uploads.load(Ordering::SeqCst), 1);
    h.engine.shutdown().await;
}
#[tokio::test]
async fn active_transfer_excludes_edits_and_shutdown_drains_receipt() {
    let (h, record, store) = prepared().await;
    store.gate.forget_permits(100);
    h.engine.send_to_store(record.id).await.unwrap();
    store.entered.notified().await;
    assert_eq!(
        h.engine.send_to_store(record.id).await.unwrap().state,
        TransferState::Sending
    );
    assert_eq!(store.imports.load(Ordering::SeqCst), 1);
    let mut edit = record.processed_images.as_ref().unwrap()[0].request.clone();
    edit.id = uuid::Uuid::new_v4();
    assert_eq!(h.engine.edit_image(edit).await.unwrap_err().kind, "busy");
    assert_eq!(
        h.engine
            .continue_batch(record.id, uuid::Uuid::new_v4())
            .await
            .unwrap_err()
            .kind,
        "busy"
    );
    let engine = h.engine.clone();
    let shutdown = tokio::spawn(async move { engine.shutdown().await });
    while !h.engine.status().await.stopping {
        tokio::task::yield_now().await;
    }
    assert!(!shutdown.is_finished());
    store.gate.add_permits(1);
    shutdown.await.unwrap();
    let status = settled(&h, record.id).await;
    assert_eq!(status.state, TransferState::Failed);
    assert_eq!(status.error.unwrap().kind, "storeInterrupted");
    assert_eq!(store.uploads.load(Ordering::SeqCst), 0);
}
