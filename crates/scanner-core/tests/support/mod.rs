use async_trait::async_trait;
use scanner_core::{
    Engine,
    backend::{CaptureEvent, CaptureOutcome, CardProcessor, ScannerBackend},
};
use scanner_files::StorageRoot;
use scanner_store::Database;
use scanner_types::{
    device::{Device, Inspection},
    error::{Result, ScannerError},
    images::ProcessedImage,
    scan::{ScanRecord, ScanRequest},
};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct Call {
    pub cancel: CancellationToken,
    pub finish: oneshot::Sender<bool>,
}
pub struct ButtonCall {
    pub cancel: CancellationToken,
    pub finish: oneshot::Sender<Result<()>>,
}
pub struct Driver {
    buttons: mpsc::UnboundedSender<ButtonCall>,
    calls: mpsc::UnboundedSender<Call>,
    inspections: mpsc::UnboundedSender<oneshot::Sender<bool>>,
    discovery: std::sync::Mutex<Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>>,
}
#[async_trait]
impl ScannerBackend for Driver {
    async fn wait_for_scan_button(&self, _: &str, cancel: CancellationToken) -> Result<()> {
        let (finish, finished) = oneshot::channel();
        self.buttons
            .send(ButtonCall { cancel, finish })
            .unwrap_or_else(|_| panic!("Button receiver closed"));
        finished.await.unwrap()
    }
    async fn devices(&self) -> Result<Vec<Device>> {
        let gate = self.discovery.lock().unwrap().take();
        if let Some((entered, release)) = gate {
            entered.send(()).unwrap();
            release.await.unwrap();
        }
        Ok(vec![Device {
            id: "fixture-driver".into(),
            name: "Test scanner".into(),
            ..Device::default()
        }])
    }
    async fn inspect(&self, _: &str) -> Result<Inspection> {
        let (tx, rx) = oneshot::channel();
        self.inspections.send(tx).unwrap();
        let mut report = Inspection::new(self.devices().await?.remove(0));
        report.session_opened = true;
        report.session_closed = rx.await.unwrap();
        let setting = |value, allowed| scanner_types::device::Setting {
            current: value,
            allowed,
            default_value: None,
            read_only: false,
            evidence: scanner_types::device::Evidence::DriverExposedNotTested,
        };
        report.units.push(scanner_types::device::UnitSnapshot {
            unit_type: 3,
            settings: [
                (
                    "resolutionDPI".into(),
                    setting(serde_json::json!(600), Some(serde_json::json!([300, 600]))),
                ),
                (
                    "supportsDuplexScanning".into(),
                    setting(serde_json::json!(true), None),
                ),
                (
                    "documentLoaded".into(),
                    setting(serde_json::json!(true), None),
                ),
            ]
            .into(),
            ..Default::default()
        });
        Ok(report)
    }
    async fn capture(
        &self,
        _: &str,
        request: &ScanRequest,
        incoming: PathBuf,
        events: mpsc::Sender<CaptureEvent>,
        cancel: CancellationToken,
    ) -> Result<CaptureOutcome> {
        let png = scanner_images::encode_png(image::RgbaImage::from_pixel(
            6,
            9,
            image::Rgba([40, 90, 160, 255]),
        ))?;
        let (acknowledged, persisted) = oneshot::channel();
        events
            .send(CaptureEvent::Settings {
                settings: Default::default(),
                acknowledged,
            })
            .await
            .unwrap();
        persisted.await.unwrap()?;
        for sequence in 1..=2 {
            let filename = format!("driver-{sequence}.png");
            std::fs::write(incoming.join(&filename), &png.bytes).unwrap();
            events
                .send(CaptureEvent::Image { filename, sequence })
                .await
                .unwrap();
        }
        let (tx, rx) = oneshot::channel();
        self.calls
            .send(Call {
                cancel: cancel.clone(),
                finish: tx,
            })
            .unwrap();
        let session_closed = rx.await.unwrap();
        Ok(CaptureOutcome {
            session_closed,
            cancelled: cancel.is_cancelled(),
            errors: if request.extensions.get("simulateCleanupFailure")
                == Some(&serde_json::json!(true))
            {
                vec![ScannerError::new(
                    "cleanup",
                    "Cancellation was not acknowledged.",
                )]
            } else {
                vec![]
            },
        })
    }
}
struct NoPreparation;
#[async_trait]
impl CardProcessor for NoPreparation {
    async fn prepare(&self, _: &ScanRecord, _: u32) -> Result<ProcessedImage> {
        Err(ScannerError::new("test", "Preparation was not requested."))
    }
}
pub struct Harness {
    driver: Arc<Driver>,
    pub engine: Arc<Engine>,
    pub calls: mpsc::UnboundedReceiver<Call>,
    pub inspections: mpsc::UnboundedReceiver<oneshot::Sender<bool>>,
    pub buttons: mpsc::UnboundedReceiver<ButtonCall>,
    pub directory: tempfile::TempDir,
}
impl Harness {
    pub fn new() -> Self {
        Self::with_recognizer(None)
    }
    pub fn with_recognizer(
        recognizer: Option<Arc<dyn scanner_core::backend::CardRecognizer>>,
    ) -> Self {
        Self::with_components(Arc::new(NoPreparation), recognizer)
    }
    pub fn with_components(
        processor: Arc<dyn CardProcessor>,
        recognizer: Option<Arc<dyn scanner_core::backend::CardRecognizer>>,
    ) -> Self {
        Self::with_components_at(processor, recognizer, None)
    }
    pub fn with_storage(
        recognizer: Option<Arc<dyn scanner_core::backend::CardRecognizer>>,
        storage: StorageRoot,
    ) -> Self {
        Self::with_components_at(Arc::new(NoPreparation), recognizer, Some(storage))
    }
    fn with_components_at(
        processor: Arc<dyn CardProcessor>,
        recognizer: Option<Arc<dyn scanner_core::backend::CardRecognizer>>,
        storage: Option<StorageRoot>,
    ) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let (tx, calls) = mpsc::unbounded_channel();
        let (button_tx, buttons) = mpsc::unbounded_channel();
        let (inspections_tx, inspections) = mpsc::unbounded_channel();
        let driver = Arc::new(Driver {
            buttons: button_tx,
            calls: tx,
            inspections: inspections_tx,
            discovery: Default::default(),
        });
        let engine = Engine::with_recognizer(
            driver.clone(),
            processor,
            recognizer,
            Arc::new(Database::memory().unwrap()),
            Arc::new(storage.unwrap_or_else(|| StorageRoot::open(directory.path()).unwrap())),
        );
        Self {
            driver,
            engine,
            calls,
            inspections,
            buttons,
            directory,
        }
    }
    pub fn block_discovery(&self) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
        let (entered, ready) = oneshot::channel();
        let (release, wait) = oneshot::channel();
        *self.driver.discovery.lock().unwrap() = Some((entered, wait));
        (ready, release)
    }
    pub async fn next(&mut self) -> Call {
        tokio::time::timeout(Duration::from_secs(3), self.calls.recv())
            .await
            .unwrap()
            .unwrap()
    }
    pub async fn terminal(&self, id: Uuid) -> ScanRecord {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let record = self.engine.scan_status(id).unwrap();
                if record.state.terminal() {
                    return record;
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap()
    }
}
