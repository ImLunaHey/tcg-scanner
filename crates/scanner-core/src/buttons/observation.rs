use super::Reservation;
use crate::Engine;
use scanner_types::error::{Result, ScannerError};
use std::sync::Arc;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

impl Engine {
    /// Bounded, explicit diagnostic ownership. Never creates a scan job.
    pub async fn start_button_observation(self: &Arc<Self>, seconds: u32) -> Result<()> {
        if !(30..=300).contains(&seconds) {
            return Err(ScannerError::new(
                "invalidParams",
                "Observation must last 30–300 seconds.",
            ));
        }
        let cancel = CancellationToken::new();
        let (done, receiver) = watch::channel(false);
        {
            let mut state = self.state.lock().await;
            state.accepting()?;
            if state.inspecting || state.camera || state.active.is_some() || !state.queue.is_empty()
            {
                return Err(busy());
            }
            state.inspecting = true;
            state.observation = Some(Reservation {
                cancel: cancel.clone(),
                done: receiver,
            });
        }
        let engine = self.clone();
        tokio::spawn(async move {
            engine.observe_owned(seconds, cancel, done).await;
        });
        Ok(())
    }
    async fn observe_owned(
        self: Arc<Self>,
        seconds: u32,
        cancel: CancellationToken,
        done: watch::Sender<bool>,
    ) {
        let engine = self.clone();
        let result = tokio::spawn(async move {
            engine.release_button().await?;
            engine.observation_admission(&cancel).await?;
            let device = engine.select_device(None).await?;
            engine.observation_admission(&cancel).await?;
            engine
                .backend
                .observe_button(&device.id, seconds, cancel)
                .await
        })
        .await
        .unwrap_or_else(|_| {
            Err(ScannerError::new(
                "cleanup",
                "Observation worker stopped without confirmed closure.",
            ))
        });
        if let Err(error) = &result
            && error.kind != "cancelled"
        {
            let _ = self.database.event(
                "diagnostics.buttonMonitor.failed",
                &serde_json::json!(error),
            );
        }
        {
            let mut state = self.state.lock().await;
            if let Err(error) = &result {
                state.quarantined |= matches!(
                    error.kind.as_str(),
                    "cleanup" | "closeSession" | "disconnected"
                );
            }
            state.observation = None;
            state.inspecting = false;
        }
        done.send_replace(true);
        self.wake.notify_one();
        self.idle.notify_waiters();
    }
    async fn observation_admission(&self, cancel: &CancellationToken) -> Result<()> {
        if cancel.is_cancelled() {
            return Err(ScannerError::new(
                "cancelled",
                "Button observation cancelled.",
            ));
        }
        let state = self.state.lock().await;
        state.accepting()?;
        if !state.queue.is_empty() {
            return Err(busy());
        }
        Ok(())
    }
    pub async fn stop_button_observation(self: &Arc<Self>) -> Result<()> {
        let reservation = self
            .state
            .lock()
            .await
            .observation
            .as_ref()
            .map(|r| (r.cancel.clone(), r.done.clone()));
        if let Some((cancel, mut done)) = reservation {
            cancel.cancel();
            tokio::spawn(async move {
                let _ = done.wait_for(|done| *done).await;
            })
            .await
            .map_err(crate::image_access::worker)?;
        }
        Ok(())
    }
}
fn busy() -> ScannerError {
    ScannerError::new("busy", "Service must be idle for button observation.")
}
