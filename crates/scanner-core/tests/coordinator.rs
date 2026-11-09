mod support;
use scanner_types::scan::{ScanRequest, ScanState};
use std::time::Duration;
use support::Harness;
use uuid::Uuid;

#[tokio::test]
async fn concurrent_retries_reserve_one_batch_and_preserve_submitted_settings() {
    let mut h = Harness::new();
    let request = ScanRequest {
        dpi: 300,
        ..ScanRequest::default()
    };
    let (first, second) = tokio::join!(
        h.engine.start_scan(request.clone()),
        h.engine.start_scan(request.clone())
    );
    let first = first.unwrap();
    assert_eq!(first.id, second.unwrap().id);
    assert_eq!(first.request.dpi, 600);
    assert_eq!(
        h.engine.database().submission(first.id).unwrap().unwrap(),
        request
    );
    let conflicting = ScanRequest {
        duplex: true,
        ..request
    };
    assert_eq!(
        h.engine.start_scan(conflicting).await.unwrap_err().kind,
        "conflict"
    );
    let call = h.next().await;
    call.finish.send(true).unwrap();
    let done = h.terminal(first.id).await;
    assert_eq!(done.state, ScanState::Completed);
    assert_eq!(done.images.len(), 2);
    assert!(h.calls.try_recv().is_err());
    assert_eq!(std::fs::read_dir(h.directory.path()).unwrap().count(), 1);
    h.engine.shutdown().await;
}

#[tokio::test]
async fn cancelling_a_capture_waits_for_closure_before_the_next_capture() {
    let mut h = Harness::new();
    let first = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    let call = h.next().await;
    let second = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    h.engine.cancel_scan(first.id).await.unwrap();
    assert!(call.cancel.is_cancelled());
    assert_eq!(h.engine.status().await.active_job_id, Some(first.id));
    assert_eq!(h.engine.status().await.queued_jobs, 1);
    assert!(h.calls.try_recv().is_err());
    call.finish.send(true).unwrap();
    let next = h.next().await;
    assert_eq!(h.terminal(first.id).await.state, ScanState::Cancelled);
    next.finish.send(true).unwrap();
    assert_eq!(h.terminal(second.id).await.state, ScanState::Completed);
    h.engine.shutdown().await;
}

#[tokio::test]
async fn unknown_session_closure_quarantines_hardware_and_fails_waiting_jobs() {
    let mut h = Harness::new();
    let first = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    let call = h.next().await;
    let second = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    call.finish.send(false).unwrap();
    assert_eq!(h.terminal(first.id).await.state, ScanState::Failed);
    assert_eq!(h.terminal(second.id).await.state, ScanState::Failed);
    assert!(h.engine.status().await.requires_restart);
    assert!(h.engine.start_scan(ScanRequest::default()).await.is_err());
    assert!(h.calls.try_recv().is_err());
    h.engine.shutdown().await;
}

#[tokio::test]
async fn continuation_keeps_originals_and_retries_do_not_feed_again() {
    let mut h = Harness::new();
    let first = h
        .engine
        .start_scan(ScanRequest {
            duplex: true,
            ..ScanRequest::default()
        })
        .await
        .unwrap();
    h.next().await.finish.send(true).unwrap();
    let before = h.terminal(first.id).await;
    let request = Uuid::new_v4();
    let (a, b) = tokio::join!(
        h.engine.continue_batch(first.id, request),
        h.engine.continue_batch(first.id, request)
    );
    assert_eq!(a.unwrap().id, b.unwrap().id);
    h.next().await.finish.send(true).unwrap();
    let after = h.terminal(first.id).await;
    assert_eq!(after.images.len(), 4);
    assert_eq!(after.images[0].sha256, before.images[0].sha256);
    assert_eq!(after.images[3].sequence, 4);
    assert_eq!(after.extensions["continuations"][0]["imagesSaved"], 2);
    assert!(h.calls.try_recv().is_err());
    h.engine.shutdown().await;
}

#[tokio::test]
async fn shutdown_drains_waiting_jobs_and_waits_for_active_cleanup() {
    let mut h = Harness::new();
    let first = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    let call = h.next().await;
    let second = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    let engine = h.engine.clone();
    let shutdown = tokio::spawn(async move { engine.shutdown().await });
    tokio::time::timeout(Duration::from_secs(3), call.cancel.cancelled())
        .await
        .unwrap();
    assert!(!shutdown.is_finished());
    call.finish.send(true).unwrap();
    shutdown.await.unwrap();
    assert_eq!(
        h.engine.scan_status(first.id).unwrap().state,
        ScanState::Cancelled
    );
    assert_eq!(
        h.engine.scan_status(second.id).unwrap().state,
        ScanState::Cancelled
    );
    assert!(h.engine.start_scan(ScanRequest::default()).await.is_err());
    assert!(h.calls.try_recv().is_err());
}

#[tokio::test]
async fn inspection_outlives_disconnected_caller_and_blocks_capture_until_closed() {
    let mut h = Harness::new();
    let engine = h.engine.clone();
    let caller = tokio::spawn(async move { engine.inspect(None).await });
    let close = h.inspections.recv().await.unwrap();
    caller.abort();
    let job = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    assert_eq!(h.engine.status().await.queued_jobs, 1);
    assert!(h.calls.try_recv().is_err());
    close.send(true).unwrap();
    h.next().await.finish.send(true).unwrap();
    assert_eq!(h.terminal(job.id).await.state, ScanState::Completed);
    assert!(
        h.engine
            .database()
            .inspection("fixture-driver")
            .unwrap()
            .is_some()
    );
    h.engine.shutdown().await;
}

#[tokio::test]
async fn invalid_inspection_device_does_not_quarantine_an_unopened_session() {
    let h = Harness::new();
    assert_eq!(
        h.engine
            .inspect(Some("missing".into()))
            .await
            .unwrap_err()
            .kind,
        "deviceNotFound"
    );
    assert!(!h.engine.status().await.requires_restart);
    h.engine.shutdown().await;
}

#[tokio::test]
async fn disconnected_submitter_does_not_strand_the_accepted_batch() {
    let mut h = Harness::new();
    let (entered, release) = h.block_discovery();
    let request = ScanRequest::default();
    let id = request.request_id;
    let engine = h.engine.clone();
    let caller = tokio::spawn(async move { engine.start_scan(request).await });
    entered.await.unwrap();
    caller.abort();
    release.send(()).unwrap();
    h.next().await.finish.send(true).unwrap();
    assert_eq!(h.terminal(id).await.state, ScanState::Completed);
    h.engine.shutdown().await;
}
