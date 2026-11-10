use super::support::{Harness, json};
use axum::body::Body;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn validates_authority_origin_credentials_and_body_before_device_access() {
    let h = Harness::new();
    for (header, value, status) in [
        ("host", "evil.example", 403),
        ("origin", "https://evil.example", 403),
        ("cookie", "session=bad", 400),
        ("content-encoding", "gzip", 415),
    ] {
        let mut request = h.request("GET", "/api/v1/devices", Body::empty(), true);
        request.headers_mut().insert(header, value.parse().unwrap());
        let response = h.send(request).await;
        assert_eq!(response.status(), status);
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    }
    let mut request = h.request("GET", "/api/v1/devices", Body::empty(), true);
    request
        .headers_mut()
        .append("authorization", "Bearer duplicate".parse().unwrap());
    assert_eq!(h.send(request).await.status(), 400);
    assert_eq!(
        h.send(h.request("GET", "/api/v1/devices?token=ignored", Body::empty(), false))
            .await
            .status(),
        401
    );
    assert_eq!(
        h.send(h.request("POST", "/rpc", Body::from(vec![b' '; 65537]), true))
            .await
            .status(),
        413
    );
    assert_eq!(
        h.send(h.request("GET", "/api/v1/devices", Body::from("{}"), true))
            .await
            .status(),
        400
    );
    assert_eq!(h.driver.calls.load(Ordering::SeqCst), 0);
    let response = h
        .send(h.request("GET", "/api/v1/devices", Body::empty(), true))
        .await;
    assert_eq!(response.status(), 200);
    assert_eq!(json(response).await["devices"], serde_json::json!([]));
    assert_eq!(h.driver.calls.load(Ordering::SeqCst), 1);
    h.engine.shutdown().await;
}
#[tokio::test]
async fn session_creation_requires_root_and_revocation_takes_effect() {
    let h = Harness::new();
    let response = h
        .call("POST", "/api/v1/session", serde_json::json!({}))
        .await;
    assert_eq!(response.status(), 201);
    let session = json(response).await["token"].as_str().unwrap().to_owned();
    let request = |method, path| {
        let mut r = h.request(method, path, Body::empty(), false);
        r.headers_mut().insert(
            "authorization",
            format!("Bearer {session}").parse().unwrap(),
        );
        r
    };
    assert_eq!(h.send(request("GET", "/api/v1/status")).await.status(), 200);
    assert_eq!(
        h.send(request("POST", "/api/v1/session")).await.status(),
        403
    );
    assert_eq!(
        h.send(request("DELETE", "/api/v1/session")).await.status(),
        200
    );
    assert_eq!(h.send(request("GET", "/api/v1/status")).await.status(), 401);
    h.engine.shutdown().await;
}
#[tokio::test]
async fn active_requests_remain_charged_until_response_body_is_dropped() {
    let h = Harness::new();
    let mut responses = vec![];
    for _ in 0..16 {
        let r = h
            .send(h.request("GET", "/api/v1/status", Body::empty(), true))
            .await;
        assert_eq!(r.status(), 200);
        responses.push(r);
    }
    assert_eq!(
        h.send(h.request("GET", "/api/v1/status", Body::empty(), true))
            .await
            .status(),
        429
    );
    responses.pop();
    assert_eq!(
        h.send(h.request("GET", "/api/v1/status", Body::empty(), true))
            .await
            .status(),
        200
    );
    drop(responses);
    h.engine.shutdown().await;
}
