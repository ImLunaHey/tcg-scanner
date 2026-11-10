use crate::Engine;
use scanner_types::{
    error::{Result, ScannerError},
    scan::ScanRequest,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

pub(crate) struct Reservation {
    pub cancel: CancellationToken,
    pub done: watch::Receiver<bool>,
}
impl Engine {
    /// Opt-in configuration: physical button presses create prepared duplex jobs.
    /// The backend must already expose an enabled button monitor.
    pub fn configure_scan_button(&self, device: String) -> Result<()> {
        if device.is_empty() || device.len() > 512 || device.contains('\0') {
            return Err(ScannerError::new(
                "invalidParams",
                "Invalid scan-button device ID.",
            ));
        }
        self.scan_button.set(device).map_err(|_| {
            ScannerError::new("configuration", "Scan-button device is already configured.")
        })
    }
    pub async fn maintain_scan_button(self: &Arc<Self>) {
        let Some(device) = self.scan_button.get().cloned() else {
            return;
        };
        {
            let state = self.state.lock().await;
            if !idle(&state) {
                return;
            }
        }
        if !self.storage_status().await.is_ok_and(|s| s.available) {
            return;
        }
        let cancel = CancellationToken::new();
        let (done, receiver) = watch::channel(false);
        let epoch = {
            let mut state = self.state.lock().await;
            if !idle(&state) {
                return;
            }
            state.button = Some(Reservation {
                cancel: cancel.clone(),
                done: receiver,
            });
            state.button_epoch
        };
        let engine = self.clone();
        tokio::spawn(async move {
            engine.monitor_button(device, cancel, done, epoch).await;
        });
    }
    async fn monitor_button(
        self: Arc<Self>,
        device: String,
        cancel: CancellationToken,
        done: watch::Sender<bool>,
        epoch: u64,
    ) {
        let backend = self.backend.clone();
        let id = device.clone();
        let token = cancel.clone();
        let result = tokio::spawn(async move { backend.wait_for_scan_button(&id, token).await })
            .await
            .unwrap_or_else(|_| {
                Err(ScannerError::new(
                    "cleanup",
                    "Button-monitor worker stopped without confirmed cleanup.",
                ))
            });
        if let Err(error) = &result
            && error.kind != "cancelled"
        {
            let _ = self
                .database
                .event("scanner.button.failed", &serde_json::json!(error));
        }
        {
            let mut state = self.state.lock().await;
            if let Err(error) = &result {
                state.quarantined |= matches!(
                    error.kind.as_str(),
                    "cleanup" | "closeSession" | "disconnected"
                );
            }
            state.button = None;
            state.next_button_attempt = Some(Instant::now() + Duration::from_secs(5));
        }
        done.send_replace(true);
        self.wake.notify_one();
        self.idle.notify_waiters();
        if result.is_ok() && !cancel.is_cancelled() {
            let request = ScanRequest {
                device_id: Some(device.clone()),
                duplex: true,
                prepare_cards: Some(true),
                ..Default::default()
            };
            let _ = self.submit_button(request, epoch).await;
        }
    }
    pub(crate) async fn release_button(&self) -> Result<()> {
        let waiting = {
            let mut state = self.state.lock().await;
            state.button_epoch = state.button_epoch.wrapping_add(1);
            state
                .button
                .as_ref()
                .map(|b| (b.cancel.clone(), b.done.clone()))
        };
        if let Some((cancel, mut done)) = waiting {
            cancel.cancel();
            let _ = done.wait_for(|done| *done).await;
        }
        if self.state.lock().await.quarantined {
            return Err(ScannerError::new(
                "cleanup",
                "Scanner ownership is uncertain; reconnect before opening another session.",
            ));
        }
        Ok(())
    }
}
fn idle(state: &crate::engine::State) -> bool {
    state.accepting().is_ok()
        && state.active.is_none()
        && state.queue.is_empty()
        && !state.camera
        && !state.inspecting
        && state.button.is_none()
        && state
            .next_button_attempt
            .is_none_or(|time| Instant::now() >= time)
}
