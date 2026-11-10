use crate::support::Harness;
use scanner_types::scan::{ScanRequest, ScanState};
use std::time::Duration;

#[tokio::test]
async fn pause_outlives_caller_cancels_queue_and_resume_waits_for_closure() {
    let mut h = Harness::new();
    let first = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    let call = h.next().await;
    let queued = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    let engine = h.engine.clone();
    let pause = tokio::spawn(async move { engine.pause().await });
    tokio::time::timeout(Duration::from_secs(3), call.cancel.cancelled())
        .await
        .unwrap();
    pause.abort();
    assert!(h.engine.status().await.paused);
    assert!(h.engine.start_scan(ScanRequest::default()).await.is_err());
    let engine = h.engine.clone();
    let resume = tokio::spawn(async move { engine.resume().await });
    tokio::task::yield_now().await;
    assert!(!resume.is_finished());
    call.finish.send(true).unwrap();
    assert!(
        !tokio::time::timeout(Duration::from_secs(3), resume)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .paused
    );
    assert_eq!(h.terminal(first.id).await.state, ScanState::Cancelled);
    assert_eq!(h.terminal(queued.id).await.state, ScanState::Cancelled);
    assert!(h.calls.try_recv().is_err());
    let next = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    h.next().await.finish.send(true).unwrap();
    assert_eq!(h.terminal(next.id).await.state, ScanState::Completed);
    h.engine.shutdown().await;
}

#[tokio::test]
async fn pause_waits_for_inspection_and_wake_does_not_clear_quarantine() {
    let mut h = Harness::new();
    let engine = h.engine.clone();
    let inspection = tokio::spawn(async move { engine.inspect(None).await });
    let close = h.inspections.recv().await.unwrap();
    let engine = h.engine.clone();
    let pause = tokio::spawn(async move { engine.pause().await });
    tokio::time::timeout(Duration::from_secs(3), async {
        while !h.engine.status().await.paused {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!pause.is_finished());
    close.send(false).unwrap();
    inspection.await.unwrap().unwrap();
    assert!(pause.await.unwrap().unwrap().paused);
    assert!(h.engine.resume().await.unwrap().requires_restart);
    assert!(h.engine.start_scan(ScanRequest::default()).await.is_err());
    assert!(h.calls.try_recv().is_err());
    h.engine.shutdown().await;
    assert!(h.engine.resume().await.is_err());
}

#[tokio::test]
async fn pause_during_submission_discovery_never_feeds_after_wake() {
    let h = Harness::new();
    let (entered, release) = h.block_discovery();
    let engine = h.engine.clone();
    let submit = tokio::spawn(async move { engine.start_scan(ScanRequest::default()).await });
    entered.await.unwrap();
    let engine = h.engine.clone();
    let pause = tokio::spawn(async move { engine.pause().await });
    tokio::time::timeout(Duration::from_secs(3), async {
        while !h.engine.status().await.paused {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    release.send(()).unwrap();
    assert!(submit.await.unwrap().is_err());
    pause.await.unwrap().unwrap();
    h.engine.resume().await.unwrap();
    assert_eq!(h.engine.database().jobs(50, 0).unwrap().len(), 0);
    h.engine.shutdown().await;
}

#[tokio::test]
async fn wake_preserves_manual_pause_and_resume_cannot_override_sleep() {
    let h = Harness::new();
    h.engine.pause().await.unwrap();
    h.engine.prepare_for_sleep().await.unwrap();
    assert!(h.engine.resume_after_wake().await.unwrap().paused);
    assert!(!h.engine.resume().await.unwrap().paused);
    h.engine.prepare_for_sleep().await.unwrap();
    assert!(h.engine.resume().await.unwrap().paused);
    assert!(!h.engine.resume_after_wake().await.unwrap().paused);
    h.engine.shutdown().await;
}
