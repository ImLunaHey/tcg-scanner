use async_trait::async_trait;
use scanner_types::{
    Extensions,
    device::{Device, Inspection},
    error::{Result, ScannerError},
    scan::ScanRequest,
};
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

pub enum CaptureEvent {
    Settings {
        settings: Extensions,
        acknowledged: oneshot::Sender<Result<()>>,
    },
    Image {
        filename: String,
        sequence: u32,
    },
}
pub struct CaptureOutcome {
    pub errors: Vec<ScannerError>,
    pub cancelled: bool,
    pub session_closed: bool,
}

#[async_trait]
pub trait ScannerBackend: Send + Sync {
    async fn devices(&self) -> Result<Vec<Device>>;
    async fn inspect(&self, device_id: &str) -> Result<Inspection>;
    /// Cancellation is cooperative: return only after cleanup and report whether closure
    /// was confirmed. The coordinator never drops this future to abandon a session.
    async fn capture(
        &self,
        device_id: &str,
        request: &ScanRequest,
        incoming: PathBuf,
        events: mpsc::Sender<CaptureEvent>,
        cancel: CancellationToken,
    ) -> Result<CaptureOutcome>;
    async fn recover_after_disconnect(&self) -> bool {
        false
    }
    async fn wait_for_scan_button(
        &self,
        _device_id: &str,
        _cancel: CancellationToken,
    ) -> Result<()> {
        Err(ScannerError::new(
            "unsupported",
            "This driver does not expose a scan-button monitor.",
        ))
    }
    async fn observe_button(
        &self,
        _device_id: &str,
        _seconds: u32,
        _cancel: CancellationToken,
    ) -> Result<()> {
        Err(ScannerError::new(
            "unsupported",
            "This driver does not expose button diagnostics.",
        ))
    }
}

/// Drivers must await this acknowledgement before issuing the physical scan
/// command. Merely queueing a settings event is not proof of durable persistence.
pub async fn persist_settings(
    events: &mpsc::Sender<CaptureEvent>,
    settings: Extensions,
) -> Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let (acknowledged, persisted) = oneshot::channel();
        events
            .send(CaptureEvent::Settings {
                settings,
                acknowledged,
            })
            .await
            .map_err(|_| {
                ScannerError::new(
                    "storage",
                    "Coordinator disconnected before settings persistence.",
                )
            })?;
        persisted.await.map_err(|_| {
            ScannerError::new("storage", "Settings persistence was not acknowledged.")
        })?
    })
    .await
    .map_err(|_| {
        ScannerError::new(
            "timeout",
            "Settings persistence timed out; scan not started.",
        )
    })?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn settings_acknowledgement_is_required_before_driver_can_continue() {
        let (tx, mut rx) = mpsc::channel(1);
        let driver = tokio::spawn(async move { persist_settings(&tx, Extensions::new()).await });
        let CaptureEvent::Settings { acknowledged, .. } = rx.recv().await.unwrap() else {
            panic!("Expected settings")
        };
        assert!(!driver.is_finished());
        acknowledged
            .send(Err(ScannerError::new("storage", "Disk full")))
            .unwrap();
        assert_eq!(driver.await.unwrap().unwrap_err().kind, "storage");
        let (tx, mut rx) = mpsc::channel(1);
        let driver = tokio::spawn(async move { persist_settings(&tx, Extensions::new()).await });
        let CaptureEvent::Settings { acknowledged, .. } = rx.recv().await.unwrap() else {
            panic!("Expected settings")
        };
        acknowledged.send(Ok(())).unwrap();
        assert!(driver.await.unwrap().is_ok());
    }
}
