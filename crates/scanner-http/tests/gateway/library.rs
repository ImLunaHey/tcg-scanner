use super::support::{Harness, json};
use axum::body::Body;
use serde_json::json;
use uuid::Uuid;
const UNEXPECTED_DESTINATION: &str = "https://evil.example";
#[tokio::test]
async fn store_status_is_authenticated_and_sends_require_a_configured_destination() {
    let h = Harness::new();
    let (job, _) = h.saved();
    let path = format!("/api/v1/batches/{job}/store");
    assert_eq!(
        h.send(h.request("GET", &path, Body::empty(), false))
            .await
            .status(),
        401
    );
    let status = json(h.send(h.request("GET", &path, Body::empty(), true)).await).await;
    assert_eq!(status["state"], "notSent");
    assert_eq!(status["configured"], false);
    assert_eq!(
        h.send(h.request("GET", &format!("{path}?extra=1"), Body::empty(), true))
            .await
            .status(),
        400
    );
    assert_eq!(
        h.call("POST", &path, json!({"destination":UNEXPECTED_DESTINATION}))
            .await
            .status(),
        400
    );
    let response = json(
        h.call(
            "POST",
            "/rpc",
            json!({"jsonrpc":"2.0","id":9,"method":"v1.batches.sendToStore","params":{"id":job}}),
        )
        .await,
    )
    .await;
    assert!(response["error"].is_object());
    assert!(h.engine.database().store_transfer(job).unwrap().is_none());
    h.engine.shutdown().await;
}
#[tokio::test]
async fn rest_and_rpc_share_profiles_validation_and_conflicts() {
    let h = Harness::new();
    let id = Uuid::new_v4();
    let created = h
        .call("POST", "/api/v1/profiles", json!({"id":id,"name":"Cards"}))
        .await;
    assert_eq!(created.status(), 201);
    assert_eq!(json(created).await["dpi"], 600);
    let conflict = h
        .call(
            "POST",
            "/api/v1/profiles",
            json!({"id":Uuid::new_v4(),"name":"CARDS"}),
        )
        .await;
    assert_eq!(conflict.status(), 409);
    let updated = h
        .call(
            "PATCH",
            &format!("/api/v1/profiles/{id}"),
            json!({"duplex":true}),
        )
        .await;
    assert_eq!(updated.status(), 200);
    let rpc = h
        .call(
            "POST",
            "/rpc",
            json!({"jsonrpc":"2.0","id":17,"method":"v1.profiles.list"}),
        )
        .await;
    let rpc = json(rpc).await;
    assert_eq!(rpc["id"], 17);
    assert_eq!(rpc["result"]["profiles"][0]["duplex"], true);
    assert_eq!(
        h.send(h.request("GET", "/api/v1/jobs?limit=1&limit=2", Body::empty(), true))
            .await
            .status(),
        400
    );
    assert_eq!(
        h.call("POST", "/api/v1/jobs", json!({"dpi":600}))
            .await
            .status(),
        400
    );
    let mut mismatch = h.request(
        "POST",
        "/api/v1/jobs",
        Body::from(json!({"requestID":id}).to_string()),
        true,
    );
    mismatch.headers_mut().insert(
        "idempotency-key",
        Uuid::new_v4().to_string().parse().unwrap(),
    );
    assert_eq!(h.send(mismatch).await.status(), 400);
    h.engine.shutdown().await;
}
#[tokio::test]
async fn image_edits_publish_through_rest_and_metadata_remains_authenticated() {
    let h = Harness::new();
    let (job, _) = h.saved();
    let url = format!("/api/v1/images/{job}:1");
    assert_eq!(
        h.send(h.request("GET", &format!("{url}/metadata"), Body::empty(), false))
            .await
            .status(),
        401
    );
    let edit = Uuid::new_v4();
    let response = h
        .call(
            "POST",
            &format!("{url}/edits"),
            json!({"id":edit,"crop":{"x":0,"y":0,"width":1,"height":1},"quarterTurns":1}),
        )
        .await;
    assert_eq!(response.status(), 201);
    assert_eq!(json(response).await["image"]["width"], 9);
    let metadata = h
        .send(h.request(
            "GET",
            &format!("{url}/metadata?editID={edit}"),
            Body::empty(),
            true,
        ))
        .await;
    assert_eq!(json(metadata).await["image"]["width"], 9);
    let bad = h
        .call(
            "POST",
            &format!("{url}/edits"),
            json!({"id":Uuid::new_v4(),"crop":{"x":0,"y":0,"width":2,"height":1},"quarterTurns":0}),
        )
        .await;
    assert_eq!(bad.status(), 400);
    h.engine.shutdown().await;
}
