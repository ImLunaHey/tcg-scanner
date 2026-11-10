use crate::support::Harness;
use scanner_types::{
    error::ScannerError,
    scan::{ScanRequest, ScanState},
};
use std::time::Duration;

#[tokio::test]
async fn physical_button_press_creates_one_prepared_duplex_job_and_debounces() {
    let mut h = Harness::new();
    h.engine
        .configure_scan_button("fixture-driver".into())
        .unwrap();
    h.engine.maintain_scan_button().await;
    let button = h.buttons.recv().await.unwrap();
    assert!(!button.cancel.is_cancelled());
    assert!(!button.observation);
    button.finish.send(Ok(())).unwrap();
    let capture = h.next().await;
    let id = h.engine.status().await.active_job_id.unwrap();
    let record = h.engine.scan_status(id).unwrap();
    assert!(record.request.duplex);
    assert_eq!(record.request.prepare_cards, Some(true));
    capture.finish.send(true).unwrap();
    assert_eq!(h.terminal(id).await.state, ScanState::Completed);
    h.engine.maintain_scan_button().await;
    assert!(h.buttons.try_recv().is_err());
    assert_eq!(h.engine.jobs(50, 0).await.unwrap().jobs.len(), 1);
    h.engine.shutdown().await;
}
#[tokio::test]
async fn scan_handoff_waits_for_button_cleanup_and_does_not_duplicate_a_late_press() {
    let mut h = Harness::new();
    h.engine
        .configure_scan_button("fixture-driver".into())
        .unwrap();
    h.engine.maintain_scan_button().await;
    let button = h.buttons.recv().await.unwrap();
    let job = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), button.cancel.cancelled())
        .await
        .unwrap();
    assert!(h.calls.try_recv().is_err());
    button.finish.send(Ok(())).unwrap();
    h.next().await.finish.send(true).unwrap();
    assert_eq!(h.terminal(job.id).await.state, ScanState::Completed);
    assert_eq!(h.engine.jobs(50, 0).await.unwrap().jobs.len(), 1);
    h.engine.shutdown().await;
}
#[tokio::test]
async fn button_cleanup_failure_quarantines_pending_capture_before_hardware_feed() {
    let mut h = Harness::new();
    h.engine
        .configure_scan_button("fixture-driver".into())
        .unwrap();
    h.engine.maintain_scan_button().await;
    let button = h.buttons.recv().await.unwrap();
    let job = h.engine.start_scan(ScanRequest::default()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), button.cancel.cancelled())
        .await
        .unwrap();
    button
        .finish
        .send(Err(ScannerError::new(
            "closeSession",
            "Fixture cannot close",
        )))
        .unwrap();
    assert_eq!(h.terminal(job.id).await.state, ScanState::Failed);
    assert!(h.engine.status().await.requires_restart);
    assert!(h.calls.try_recv().is_err());
    h.engine.shutdown().await;
}
#[tokio::test]
async fn sleep_cancels_the_button_listener_and_waits_for_session_close() {
    let mut h = Harness::new();
    h.engine
        .configure_scan_button("fixture-driver".into())
        .unwrap();
    h.engine.maintain_scan_button().await;
    let button = h.buttons.recv().await.unwrap();
    let engine = h.engine.clone();
    let sleep = tokio::spawn(async move { engine.prepare_for_sleep().await });
    tokio::time::timeout(Duration::from_secs(3), button.cancel.cancelled())
        .await
        .unwrap();
    assert!(!sleep.is_finished());
    button.finish.send(Ok(())).unwrap();
    sleep.await.unwrap().unwrap();
    h.engine.resume_after_wake().await.unwrap();
    assert!(h.engine.jobs(50, 0).await.unwrap().jobs.is_empty());
    assert!(h.calls.try_recv().is_err());
    h.engine.shutdown().await;
}

#[tokio::test]
async fn inspection_waits_for_button_cleanup_and_shutdown_drains_an_idle_listener() {
    let mut h = Harness::new();
    h.engine
        .configure_scan_button("fixture-driver".into())
        .unwrap();
    h.engine.maintain_scan_button().await;
    let button = h.buttons.recv().await.unwrap();
    let engine = h.engine.clone();
    let inspection = tokio::spawn(async move { engine.inspect(None).await });
    tokio::time::timeout(Duration::from_secs(3), button.cancel.cancelled())
        .await
        .unwrap();
    assert!(h.inspections.try_recv().is_err());
    button.finish.send(Ok(())).unwrap();
    h.inspections.recv().await.unwrap().send(true).unwrap();
    inspection.await.unwrap().unwrap();
    assert!(h.engine.jobs(50, 0).await.unwrap().jobs.is_empty());
    h.engine.shutdown().await;

    let mut h = Harness::new();
    h.engine
        .configure_scan_button("fixture-driver".into())
        .unwrap();
    h.engine.maintain_scan_button().await;
    let button = h.buttons.recv().await.unwrap();
    let engine = h.engine.clone();
    let shutdown = tokio::spawn(async move { engine.shutdown().await });
    tokio::time::timeout(Duration::from_secs(3), button.cancel.cancelled())
        .await
        .unwrap();
    assert!(!shutdown.is_finished());
    button.finish.send(Ok(())).unwrap();
    tokio::time::timeout(Duration::from_secs(3), shutdown)
        .await
        .unwrap()
        .unwrap();
    assert!(h.engine.jobs(50, 0).await.unwrap().jobs.is_empty());
}

#[tokio::test]
async fn explicit_observation_releases_automatic_listener_and_never_feeds() {
    let mut h = Harness::new();
    h.engine
        .configure_scan_button("fixture-driver".into())
        .unwrap();
    h.engine.maintain_scan_button().await;
    let automatic = h.buttons.recv().await.unwrap();
    assert!(h.engine.start_button_observation(29).await.is_err());
    h.engine.start_button_observation(30).await.unwrap();
    assert!(h.engine.status().await.button_observation);
    tokio::time::timeout(Duration::from_secs(3), automatic.cancel.cancelled())
        .await
        .unwrap();
    assert!(h.buttons.try_recv().is_err());
    automatic.finish.send(Ok(())).unwrap();
    let observed = h.buttons.recv().await.unwrap();
    assert!(observed.observation);
    assert!(h.engine.start_button_observation(30).await.is_err());
    let engine = h.engine.clone();
    let stop = tokio::spawn(async move { engine.stop_button_observation().await });
    tokio::time::timeout(Duration::from_secs(3), observed.cancel.cancelled())
        .await
        .unwrap();
    assert!(!stop.is_finished());
    observed.finish.send(Ok(())).unwrap();
    stop.await.unwrap().unwrap();
    assert!(!h.engine.status().await.button_observation);
    assert!(h.engine.jobs(50, 0).await.unwrap().jobs.is_empty());
    assert!(h.calls.try_recv().is_err());
    h.engine.shutdown().await;
}
#[tokio::test]
async fn stopping_observation_during_discovery_cannot_open_a_late_session() {
    let mut h = Harness::new();
    let (entered, release) = h.block_discovery();
    h.engine.start_button_observation(30).await.unwrap();
    entered.await.unwrap();
    let engine = h.engine.clone();
    let stop = engine.stop_button_observation();
    tokio::pin!(stop);
    tokio::select! { biased;
        _ = &mut stop => panic!("Stop returned before discovery cleanup"),
        _ = tokio::task::yield_now() => {}
    }
    release.send(()).unwrap();
    stop.await.unwrap();
    assert!(h.buttons.try_recv().is_err());
    assert!(!h.engine.status().await.button_observation);
    h.engine.shutdown().await;
}
