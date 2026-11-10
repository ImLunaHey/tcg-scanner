use super::support::Harness;
use scanner_types::{
    scan::OriginalFormat,
    service::{ProfilePatch, ScanProfile, ScanSettings},
};
use uuid::Uuid;

#[tokio::test]
async fn profile_controls_are_persistent_and_shutdown_refuses_further_writes() {
    let h = Harness::new();
    let profile = ScanProfile {
        id: Uuid::new_v4(),
        name: "Cards".into(),
        settings: ScanSettings {
            dpi: 600,
            duplex: false,
            format: OriginalFormat::Png,
        },
    };
    assert_eq!(
        h.engine.create_profile(profile.clone()).await.unwrap(),
        profile
    );
    h.engine
        .update_profile(
            "cards".into(),
            ProfilePatch {
                duplex: Some(true),
                ..ProfilePatch::default()
            },
        )
        .await
        .unwrap();
    assert!(
        h.engine
            .profile(profile.id.to_string())
            .await
            .unwrap()
            .settings
            .duplex
    );
    assert_eq!(h.engine.profiles().await.unwrap().len(), 1);
    assert_eq!(h.engine.events(0, 100, 0).await.unwrap().events.len(), 2);
    assert!(h.engine.jobs(100, 0).await.unwrap().jobs.is_empty());
    assert!(h.engine.jobs(101, 0).await.is_err());
    h.engine.remove_profile("CARDS".into()).await.unwrap();
    assert!(h.engine.profiles().await.unwrap().is_empty());
    h.engine.shutdown().await;
    assert_eq!(
        h.engine.save_profile(profile).await.unwrap_err().kind,
        "busy"
    );
}
#[tokio::test]
async fn event_waiters_are_bounded_and_disconnected_subscribers_release_capacity() {
    let h = Harness::new();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..9 {
        let engine = h.engine.clone();
        tasks.spawn(async move { engine.events(0, 100, 20).await });
    }
    let first = tokio::time::timeout(std::time::Duration::from_secs(2), tasks.join_next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(first.unwrap_err().kind, "resourceLimit");
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    assert!(h.engine.events(0, 100, 0).await.unwrap().events.is_empty());
    let engine = h.engine.clone();
    let waiter = tokio::spawn(async move { engine.events(0, 100, 20).await });
    h.engine
        .database()
        .event("test", &serde_json::json!({"changed":true}))
        .unwrap();
    let page = tokio::time::timeout(std::time::Duration::from_secs(2), waiter)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(page.events[0].kind, "test");
    h.engine.shutdown().await;
}
mod condition;
mod images;
mod settings;

mod identification;

mod preparation;

mod finish;

mod pricing;

mod staging;

mod camera;
