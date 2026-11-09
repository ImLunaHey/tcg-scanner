use async_trait::async_trait;
use scanner_core::{
    Engine,
    backend::{CaptureEvent, CaptureOutcome, CardProcessor, ScannerBackend},
};
use scanner_files::StorageRoot;
use scanner_rpc::{Response, Router};
use scanner_store::Database;
use scanner_types::{
    device::{Device, Inspection},
    error::{Result, ScannerError},
    images::ProcessedImage,
    scan::{ScanRecord, ScanRequest},
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Default)]
struct Driver {
    discovery: AtomicUsize,
    captures: AtomicUsize,
}
#[async_trait]
impl ScannerBackend for Driver {
    async fn devices(&self) -> Result<Vec<Device>> {
        self.discovery.fetch_add(1, Ordering::SeqCst);
        Ok(vec![Device {
            id: "fixture".into(),
            name: "Test scanner".into(),
            ..Device::default()
        }])
    }
    async fn inspect(&self, _: &str) -> Result<Inspection> {
        let mut result = Inspection::new(self.devices().await?.remove(0));
        result.session_opened = true;
        result.session_closed = true;
        Ok(result)
    }
    async fn capture(
        &self,
        _: &str,
        _: &ScanRequest,
        _: PathBuf,
        _: mpsc::Sender<CaptureEvent>,
        cancel: CancellationToken,
    ) -> Result<CaptureOutcome> {
        self.captures.fetch_add(1, Ordering::SeqCst);
        cancel.cancelled().await;
        Ok(CaptureOutcome {
            errors: vec![],
            cancelled: true,
            session_closed: true,
        })
    }
}
struct Processor;
#[async_trait]
impl CardProcessor for Processor {
    async fn prepare(&self, _: &ScanRecord, _: u32) -> Result<ProcessedImage> {
        Err(ScannerError::new(
            "test",
            "No images in this protocol fixture.",
        ))
    }
}
struct Harness {
    router: Router,
    engine: Arc<Engine>,
    driver: Arc<Driver>,
    _directory: tempfile::TempDir,
}
impl Harness {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let driver = Arc::new(Driver::default());
        let engine = Engine::new(
            driver.clone(),
            Arc::new(Processor),
            Arc::new(Database::memory().unwrap()),
            Arc::new(StorageRoot::open(directory.path()).unwrap()),
        );
        Self {
            router: Router::new(engine.clone(), "Test"),
            engine,
            driver,
            _directory: directory,
        }
    }
    async fn call(&self, method: &str, params: Value) -> Response {
        self.router
            .respond(
                &serde_json::to_vec(
                    &json!({"jsonrpc":"2.0","id":"request","method":method,"params":params}),
                )
                .unwrap(),
            )
            .await
    }
}
#[tokio::test]
async fn rejects_malformed_and_unsupported_requests_before_touching_hardware() {
    let h = Harness::new();
    for (bytes, code) in [
        (b"[".as_slice(), -32700),
        (b"[]".as_slice(), -32600),
        (
            br#"{"jsonrpc":"1.0","id":true,"method":"v1.devices"}"#,
            -32600,
        ),
    ] {
        assert_eq!(h.router.respond(bytes).await.error.unwrap().code, code);
    }
    assert_eq!(
        h.router
            .respond(&vec![b' '; 65537])
            .await
            .error
            .unwrap()
            .code,
        -32600
    );
    assert_eq!(
        h.call("v1.devices", json!({"deviceID":"fixture"}))
            .await
            .error
            .unwrap()
            .code,
        -32602
    );
    let invalid_scan = json!({
        "requestID": Uuid::new_v4(), "dpi": 600, "duplex": false,
        "format": "png", "unexpected": true
    });
    assert_eq!(
        h.call("v1.scan.start", invalid_scan)
            .await
            .error
            .unwrap()
            .code,
        -32602
    );
    assert_eq!(
        h.call("v1.service.status", json!([]))
            .await
            .error
            .unwrap()
            .code,
        -32602
    );
    assert_eq!(
        h.call("v1.unknown", json!({})).await.error.unwrap().code,
        -32601
    );
    assert_eq!(h.driver.discovery.load(Ordering::SeqCst), 0);
    assert_eq!(h.driver.captures.load(Ordering::SeqCst), 0);
    h.engine.shutdown().await;
}
#[tokio::test]
async fn profile_protocol_preserves_envelopes_conflicts_and_partial_updates() {
    let h = Harness::new();
    let id = Uuid::new_v4();
    let params = json!({"id":id,"name":"Cards","dpi":600,"duplex":false,"format":"png"});
    let response = h.call("v1.profiles.create", params.clone()).await;
    assert_eq!(response.id, "request");
    assert!(response.error.is_none());
    assert_eq!(response.result.unwrap()["id"], id.to_string());
    assert!(h.call("v1.profiles.create", params).await.error.is_none());
    assert_eq!(
        h.call(
            "v1.profiles.update",
            json!({"id":"cards","name":null,"duplex":true})
        )
        .await
        .error
        .unwrap()
        .code,
        -32602
    );
    let updated = h
        .call("v1.profiles.update", json!({"id":"cards","duplex":true}))
        .await
        .result
        .unwrap();
    assert_eq!(updated["duplex"], true);
    assert_eq!(updated["dpi"], 600);
    let duplicate = h
        .call(
            "v1.profiles.create",
            json!({"id":Uuid::new_v4(),"name":"CARDS","dpi":600,"duplex":false,"format":"png"}),
        )
        .await
        .error
        .unwrap();
    assert_eq!(duplicate.code, -32000);
    assert_eq!(duplicate.data.unwrap().kind, "conflict");
    assert_eq!(
        h.call("v1.profiles.list", json!({})).await.result.unwrap()["profiles"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        h.call("v1.profiles.delete", json!({"id":id}))
            .await
            .error
            .is_none()
    );
    let response = h.call("v1.profiles.get", json!({"id":id})).await;
    let encoded = serde_json::to_value(response).unwrap();
    assert!(encoded.get("result").is_none());
    assert!(encoded["error"]["data"].is_object());
    h.engine.shutdown().await;
}
#[tokio::test]
async fn scan_aliases_reuse_idempotency_and_cancellation_owns_cleanup() {
    let h = Harness::new();
    let id = Uuid::new_v4();
    let params =
        json!({"requestID":id,"dpi":600,"duplex":false,"format":"png","prepareCards":false});
    let first = h.call("v1.scan.start", params.clone()).await;
    assert_eq!(first.result.unwrap()["id"], id.to_string());
    assert!(h.call("v1.jobs.create", params).await.error.is_none());
    tokio::time::timeout(Duration::from_secs(2), async {
        while h.driver.captures.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let page = h
        .call("v1.batches.list", json!({"limit":1.0}))
        .await
        .result
        .unwrap();
    assert_eq!(page["jobs"].as_array().unwrap().len(), 1);
    assert!(
        h.call("v1.scan.cancel", json!({"scanID":id}))
            .await
            .error
            .is_none()
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let record = h
                .call("v1.jobs.get", json!({"id":id}))
                .await
                .result
                .unwrap();
            if record["state"] == "cancelled" {
                assert_eq!(record["sessionClosed"], true);
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(h.driver.captures.load(Ordering::SeqCst), 1);
    h.engine.shutdown().await;
}
