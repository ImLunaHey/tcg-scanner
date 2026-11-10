use super::support::{Harness, json};
use axum::body::Body;
use serde_json::json;

#[tokio::test]
async fn lifecycle_controls_require_authority_and_share_rpc_state() {
    let h = Harness::new();
    for action in ["pause", "resume"] {
        let path = format!("/api/v1/service/{action}");
        assert_eq!(
            h.send(h.request("POST", &path, Body::from("{}"), false))
                .await
                .status(),
            401
        );
        assert_eq!(
            h.call("POST", &path, json!({"extra":true})).await.status(),
            400
        );
        assert_eq!(
            h.call("POST", &format!("{path}?extra=1"), json!({}))
                .await
                .status(),
            400
        );
    }
    assert!(!h.engine.status().await.paused);
    let paused = json(h.call("POST", "/api/v1/service/pause", json!({})).await).await;
    assert_eq!(paused["paused"], true);
    let resumed = json(
        h.call(
            "POST",
            "/rpc",
            json!({"jsonrpc":"2.0","id":1,"method":"v1.service.resume","params":{}}),
        )
        .await,
    )
    .await;
    assert_eq!(resumed["result"]["paused"], false);
    assert_eq!(h.driver.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    h.engine.shutdown().await;
}

#[tokio::test]
async fn storage_reads_and_reconnect_are_authenticated_and_create_no_files() {
    let h = Harness::new();
    for (method, path) in [
        ("GET", "/api/v1/storage"),
        ("POST", "/api/v1/storage/reconnect"),
    ] {
        assert_eq!(
            h.send(h.request(method, path, Body::empty(), false))
                .await
                .status(),
            401
        );
    }
    let before = std::fs::read_dir(h.engine.storage().path())
        .unwrap()
        .count();
    let status = json(
        h.send(h.request("GET", "/api/v1/storage", Body::empty(), true))
            .await,
    )
    .await;
    assert_eq!(status["available"], true);
    assert_eq!(
        h.call(
            "POST",
            "/api/v1/storage/reconnect",
            json!({"path":"/tmp/other"})
        )
        .await
        .status(),
        400
    );
    let reopened = json(h.call("POST", "/api/v1/storage/reconnect", json!({})).await).await;
    assert_eq!(reopened["storageRoot"], status["storageRoot"]);
    assert_eq!(reopened["available"], true);
    assert_eq!(
        std::fs::read_dir(h.engine.storage().path())
            .unwrap()
            .count(),
        before
    );
    assert_eq!(h.driver.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    h.engine.shutdown().await;
}
