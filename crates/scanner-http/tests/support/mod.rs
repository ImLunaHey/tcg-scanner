use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
    response::Response,
};
use scanner_core::{
    Engine,
    backend::{CaptureEvent, CaptureOutcome, CardProcessor, ScannerBackend},
};
use scanner_files::{BatchStore, StorageRoot};
use scanner_http::{Configuration, Gateway, generate_token};
use scanner_store::Database;
use scanner_types::{
    device::{Device, Inspection},
    error::{Result, ScannerError},
    images::ProcessedImage,
    scan::{ScanRecord, ScanRequest},
};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Default)]
pub struct Driver {
    pub calls: AtomicUsize,
}
#[async_trait]
impl ScannerBackend for Driver {
    async fn devices(&self) -> Result<Vec<Device>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![])
    }
    async fn inspect(&self, _: &str) -> Result<Inspection> {
        Err(ScannerError::new("test", "No scanner attached."))
    }
    async fn capture(
        &self,
        _: &str,
        _: &ScanRequest,
        _: PathBuf,
        _: mpsc::Sender<CaptureEvent>,
        _: CancellationToken,
    ) -> Result<CaptureOutcome> {
        panic!("HTTP fixture must never feed hardware")
    }
}
struct Processor;
#[async_trait]
impl CardProcessor for Processor {
    async fn prepare(&self, _: &ScanRecord, _: u32) -> Result<ProcessedImage> {
        panic!("No preparation requested")
    }
}
pub struct Harness {
    pub app: Router,
    pub gateway: Arc<Gateway>,
    pub engine: Arc<Engine>,
    pub token: String,
    pub driver: Arc<Driver>,
    pub _directory: tempfile::TempDir,
}
impl Harness {
    pub fn new() -> Self {
        Self::with_recognizer(None)
    }
    pub fn with_recognizer(
        recognizer: Option<Arc<dyn scanner_core::backend::CardRecognizer>>,
    ) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let driver = Arc::new(Driver::default());
        let engine = Engine::with_recognizer(
            driver.clone(),
            Arc::new(Processor),
            recognizer,
            Arc::new(Database::memory().unwrap()),
            Arc::new(StorageRoot::open(directory.path()).unwrap()),
        );
        let token = generate_token().unwrap();
        let gateway = Gateway::new(
            engine.clone(),
            "Test",
            &token,
            Configuration::new("127.0.0.1:8170".parse().unwrap(), false, None).unwrap(),
        )
        .unwrap();
        Self {
            app: gateway.router(),
            gateway,
            engine,
            token,
            driver,
            _directory: directory,
        }
    }
    pub fn request(
        &self,
        method: &str,
        path: &str,
        body: Body,
        authenticated: bool,
    ) -> Request<Body> {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("host", "127.0.0.1:8170")
            .header("content-type", "application/json");
        if authenticated {
            request = request.header("authorization", format!("Bearer {}", self.token));
        }
        request.body(body).unwrap()
    }
    pub async fn send(&self, request: Request<Body>) -> Response {
        self.app.clone().oneshot(request).await.unwrap()
    }
    pub async fn call(&self, method: &str, path: &str, body: Value) -> Response {
        self.send(self.request(
            method,
            path,
            Body::from(serde_json::to_vec(&body).unwrap()),
            true,
        ))
        .await
    }
    pub fn saved(&self) -> (Uuid, Vec<u8>) {
        let png = scanner_images::encode_png(image::RgbaImage::from_pixel(
            6,
            9,
            image::Rgba([10, 40, 90, 255]),
        ))
        .unwrap();
        let mut store = BatchStore::create(
            self.engine.storage(),
            Device::default(),
            ScanRequest::default(),
            1,
        )
        .unwrap();
        std::fs::write(store.incoming_path().join("driver.png"), &png.bytes).unwrap();
        store.preserve("driver.png", 1).unwrap();
        store.finish(vec![], false, true).unwrap();
        self.engine
            .database()
            .insert(store.record(), &store.record().request)
            .unwrap();
        (store.record().id, png.bytes)
    }
}
pub async fn json(response: Response) -> Value {
    serde_json::from_slice(
        &to_bytes(response.into_body(), 2 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap()
}
