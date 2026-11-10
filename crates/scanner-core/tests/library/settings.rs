use super::super::support::Harness;
use scanner_types::{scan::OriginalFormat, service::ScanSettings};

fn settings(dpi: u32) -> ScanSettings {
    ScanSettings {
        dpi,
        duplex: true,
        format: OriginalFormat::Png,
    }
}
#[tokio::test]
async fn defaults_wait_for_inspection_close_and_survive_disconnected_callers() {
    let mut h = Harness::new();
    let engine = h.engine.clone();
    let task = tokio::spawn(async move { engine.set_settings(None, settings(600)).await });
    let close = h.inspections.recv().await.unwrap();
    assert!(
        h.engine
            .configured_settings("fixture-driver".into())
            .await
            .unwrap()
            .is_none()
    );
    task.abort();
    let _ = task.await;
    close.send(true).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while h
            .engine
            .configured_settings("fixture-driver".into())
            .await
            .unwrap()
            .is_none()
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(h.calls.try_recv().is_err());
    let engine = h.engine.clone();
    let task = tokio::spawn(async move { engine.device_status(None).await });
    h.inspections.recv().await.unwrap().send(true).unwrap();
    let status = task.await.unwrap().unwrap();
    assert_eq!(status.ready, Some(true));
    assert_eq!(status.configured_settings, Some(settings(600)));
    let engine = h.engine.clone();
    let task = tokio::spawn(async move { engine.set_settings(None, settings(450)).await });
    h.inspections.recv().await.unwrap().send(true).unwrap();
    assert_eq!(task.await.unwrap().unwrap_err().kind, "unsupportedSetting");
    assert_eq!(
        h.engine
            .configured_settings("fixture-driver".into())
            .await
            .unwrap(),
        Some(settings(600))
    );
    h.engine.shutdown().await;
}
#[tokio::test]
async fn unclosed_validation_quarantines_and_keeps_previous_defaults() {
    let mut h = Harness::new();
    let engine = h.engine.clone();
    let task = tokio::spawn(async move { engine.set_settings(None, settings(600)).await });
    h.inspections.recv().await.unwrap().send(false).unwrap();
    assert_eq!(task.await.unwrap().unwrap_err().kind, "inspection");
    assert!(h.engine.status().await.requires_restart);
    assert!(
        h.engine
            .configured_settings("fixture-driver".into())
            .await
            .unwrap()
            .is_none()
    );
    let status = h.engine.device_status(None).await.unwrap();
    assert_eq!(status.ready, Some(false));
    assert!(status.inspection.is_none());
    assert!(h.calls.try_recv().is_err());
    h.engine.shutdown().await;
}
