use crate::Engine;
use scanner_types::{
    device::Inspection,
    error::{Result, ScannerError},
};
use std::sync::Arc;

impl Engine {
    pub async fn inspect(self: &Arc<Self>, device_id: Option<String>) -> Result<Inspection> {
        let engine = self.clone();
        // Own the guard beyond the lifetime of a disconnected API caller.
        tokio::spawn(async move { engine.inspect_owned(device_id).await })
            .await
            .map_err(|e| ScannerError::new("capture", e.to_string()))?
    }

    async fn inspect_owned(&self, device_id: Option<String>) -> Result<Inspection> {
        {
            let mut state = self.state.lock().await;
            state.accepting()?;
            if state.camera || state.inspecting || state.active.is_some() || !state.queue.is_empty()
            {
                return Err(ScannerError::new("busy", "Scanner is in use."));
            }
            state.inspecting = true;
        }
        // Device selection cannot open a session. Only a failed inspection leaves
        // closure unknown and requires quarantine.
        let (result, uncertain) = match self.release_button().await {
            Ok(()) => self.inspect_device(device_id).await,
            Err(error) => (Err(error), true),
        };
        {
            let mut state = self.state.lock().await;
            state.inspecting = false;
            state.quarantined |= uncertain;
        }
        self.wake.notify_one();
        self.idle.notify_waiters();
        if let Ok(report) = &result {
            self.database.save_inspection(
                &report.device.id,
                &serde_json::to_value(report)
                    .map_err(|e| ScannerError::new("storage", e.to_string()))?,
            )?;
        }
        result
    }

    async fn inspect_device(&self, device_id: Option<String>) -> (Result<Inspection>, bool) {
        match self.select_device(device_id.as_deref()).await {
            Ok(device) => {
                let backend = self.backend.clone();
                let result = tokio::spawn(async move { backend.inspect(&device.id).await })
                    .await
                    .unwrap_or_else(|e| Err(ScannerError::new("capture", e.to_string())));
                let uncertain = result
                    .as_ref()
                    .map(|r| r.session_opened && !r.session_closed)
                    .unwrap_or(true);
                (result, uncertain)
            }
            Err(error) => (Err(error), false),
        }
    }

    pub async fn recover_connection(self: &Arc<Self>) -> bool {
        let engine = self.clone();
        tokio::spawn(async move {
            {
                let mut state = engine.state.lock().await;
                if !state.quarantined {
                    return true;
                }
                if state.camera
                    || state.paused
                    || state.stopping
                    || state.active.is_some()
                    || state.inspecting
                    || state.button.is_some()
                    || !state.queue.is_empty()
                {
                    return false;
                }
                state.inspecting = true;
            }
            let backend = engine.backend.clone();
            let recovered = tokio::spawn(async move { backend.recover_after_disconnect().await })
                .await
                .unwrap_or(false);
            {
                let mut state = engine.state.lock().await;
                state.inspecting = false;
                state.quarantined = !recovered;
            }
            engine.wake.notify_one();
            engine.idle.notify_waiters();
            recovered
        })
        .await
        .unwrap_or(false)
    }
}
