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
