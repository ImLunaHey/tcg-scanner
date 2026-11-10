use crate::support::{Harness, json};
use axum::body::Body;
use base64::Engine as _;
use scanner_core::backend::CardRecognizer;
use scanner_types::{
    camera::{CameraIdentification, CameraState, MAX_REQUEST_BYTES},
    cards::CardLanguage,
    error::Result,
    images::OriginalImage,
    recognition::RecognitionEvidence,
    scan::ScanRecord,
};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[derive(Default)]
struct Recognizer {
    calls: AtomicUsize,
}
#[async_trait::async_trait]
impl CardRecognizer for Recognizer {
    async fn recognize(&self, _: &ScanRecord, _: &OriginalImage) -> Result<RecognitionEvidence> {
        panic!("No saved image")
    }
    async fn recognize_camera(
        &self,
        bytes: Vec<u8>,
        language: CardLanguage,
    ) -> Result<CameraIdentification> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(language, CardLanguage::Korean);
        assert_eq!(bytes.len(), 80 * 1024);
        Ok(CameraIdentification::empty(CameraState::NoCard))
    }
}
const ROUTE: &str = "/api/v1/camera/identify";
#[derive(Default)]
struct Blocking {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
#[async_trait::async_trait]
impl CardRecognizer for Blocking {
    async fn recognize(&self, _: &ScanRecord, _: &OriginalImage) -> Result<RecognitionEvidence> {
        panic!("No saved image")
    }
    async fn recognize_camera(&self, _: Vec<u8>, _: CardLanguage) -> Result<CameraIdentification> {
        self.entered.notify_one();
        self.release.notified().await;
        Ok(CameraIdentification::empty(CameraState::NoCard))
    }
}
#[tokio::test]
async fn camera_concurrency_stays_bounded_after_http_disconnect() {
    use tower::ServiceExt;
    let recognizer = Arc::new(Blocking::default());
    let h = Harness::with_recognizer(Some(recognizer.clone()));
    let request = h.request("POST", ROUTE, Body::from(body().to_string()), false);
    let caller = tokio::spawn(h.app.clone().oneshot(request));
    recognizer.entered.notified().await;
    assert_eq!(
        h.send(h.request("POST", ROUTE, Body::from(body().to_string()), false))
            .await
            .status(),
        429
    );
    caller.abort();
    let _ = caller.await;
    assert_eq!(
        h.send(h.request("POST", ROUTE, Body::from(body().to_string()), false))
            .await
            .status(),
        409
    );
    recognizer.release.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(3), h.engine.shutdown())
        .await
        .unwrap();
    assert_eq!(h.driver.calls.load(Ordering::SeqCst), 0);
}
fn body() -> serde_json::Value {
    json!({"base64":base64::engine::general_purpose::STANDARD.encode(vec![1;80*1024]),"language":"ko"})
}

#[tokio::test]
async fn native_camera_client_uses_the_anonymous_rest_contract_over_real_tcp() {
    let h = Harness::with_recognizer(Some(Arc::new(Recognizer::default())));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let gateway = scanner_http::Gateway::new(
        h.engine.clone(),
        "camera fixture",
        &h.token,
        scanner_http::Configuration::new(address, false, None).unwrap(),
    )
    .unwrap();
    let stop = tokio_util::sync::CancellationToken::new();
    let server = {
        let stop = stop.clone();
        tokio::spawn(async move { gateway.serve(listener, stop).await })
    };
    let client = scanner_client::CameraClient::new(&format!("http://{address}")).unwrap();
    let result = client
        .identify(serde_json::from_value(body()).unwrap())
        .await
        .unwrap();
    assert_eq!(result.state, CameraState::NoCard);
    stop.cancel();
    server.await.unwrap().unwrap();
    h.engine.shutdown().await;
}
#[tokio::test]
async fn public_camera_is_narrow_bounded_and_does_not_grant_library_access() {
    let recognizer = Arc::new(Recognizer::default());
    let h = Harness::with_recognizer(Some(recognizer.clone()));
    let response = h
        .send(h.request("POST", ROUTE, Body::from(body().to_string()), false))
        .await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        json(response).await,
        json!({"schemaVersion":1,"state":"noCard","candidates":[]})
    );
    for path in ["/api/v1/batches", "/rpc", ROUTE] {
        assert_eq!(
            h.send(h.request("GET", path, Body::empty(), false))
                .await
                .status(),
            401
        );
    }
    for (params, status) in [
        (json!({"base64":"!"}), 422),
        (json!({"base64":"YQ==","language":"ja"}), 400),
        (json!({"base64":"YQ==","save":true}), 400),
    ] {
        assert_eq!(
            h.send(h.request("POST", ROUTE, Body::from(params.to_string()), false))
                .await
                .status(),
            status
        );
    }
    let response = h
        .send(h.request(
            "POST",
            ROUTE,
            Body::from(vec![b' '; MAX_REQUEST_BYTES + 1]),
            false,
        ))
        .await;
    assert_eq!(response.status(), 413);
    let mut cross_origin = h.request("POST", ROUTE, Body::from(body().to_string()), false);
    cross_origin
        .headers_mut()
        .insert("origin", "https://untrusted.example".parse().unwrap());
    assert_eq!(h.send(cross_origin).await.status(), 403);
    assert_eq!(
        h.send(h.request(
            "POST",
            &format!("{ROUTE}?save=true"),
            Body::from(body().to_string()),
            false
        ))
        .await
        .status(),
        400
    );
    assert_eq!(recognizer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(h.driver.calls.load(Ordering::SeqCst), 0);
    assert!(h.engine.jobs(100, 0).await.unwrap().jobs.is_empty());
    assert!(h.engine.events(0, 100, 0).await.unwrap().events.is_empty());
    h.engine.shutdown().await;
}
#[tokio::test]
async fn authenticated_rpc_allows_large_camera_only_and_preserves_small_method_limits() {
    let h = Harness::with_recognizer(Some(Arc::new(Recognizer::default())));
    let response = h
        .call(
            "POST",
            "/rpc",
            json!({"jsonrpc":"2.0","id":1,"method":"v1.camera.identify","params":body()}),
        )
        .await;
    assert_eq!(json(response).await["result"]["state"], "noCard");
    let response = h
        .call(
            "POST",
            "/rpc",
            json!({"jsonrpc":"2.0","id":2,"method":"v1.devices","params":body()}),
        )
        .await;
    assert_eq!(response.status(), 413);
    h.engine.shutdown().await;
}
