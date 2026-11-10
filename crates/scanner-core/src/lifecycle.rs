use crate::Engine;
use scanner_types::{
    error::{Result, ScannerError},
    service::ServiceStatus,
};
use std::sync::Arc;

impl Engine {
    /// Stop admission and cancel feeding, then drain accepted operations. The
    /// owned transition survives an API disconnect; resume cannot overtake it.
    pub async fn pause(self: &Arc<Self>) -> Result<ServiceStatus> {
        let engine = self.clone();
        tokio::spawn(async move { engine.pause_owned(false).await })
            .await
            .map_err(crate::image_access::worker)?
    }
    pub async fn prepare_for_sleep(self: &Arc<Self>) -> Result<ServiceStatus> {
        let engine = self.clone();
        tokio::spawn(async move { engine.pause_owned(true).await })
            .await
            .map_err(crate::image_access::worker)?
    }
    async fn pause_owned(&self, sleep: bool) -> Result<ServiceStatus> {
        let _transition = self.lifecycle.lock().await;
        {
            let mut state = self.state.lock().await;
            if state.stopping {
                return Err(stopping());
            }
            if sleep {
                state.sleeping = true;
            } else {
                state.manual_pause = true;
            }
            state.paused = true;
            if let Some((_, cancel)) = &state.active {
                cancel.cancel();
            }
        }
        self.wake.notify_one();
        // Submissions already doing filesystem work must settle before the
        // remaining queue is removed. New submissions observe the pause.
        let mut failure = None;
        {
            let _reservation = self.submissions.lock().await;
            let queue = std::mem::take(&mut self.state.lock().await.queue);
            for mut work in queue {
                if let Err(error) = self.finish_queued(&mut work, true, None).await {
                    failure.get_or_insert(error);
                }
            }
        }
        self.wait_idle().await;
        let mut transfer = self.transfer.subscribe();
        let _ = transfer.wait_for(Option::is_none).await;
        if let Some(error) = failure {
            return Err(error);
        }
        self.database.event(
            if sleep {
                "service.sleep"
            } else {
                "service.pause"
            },
            &serde_json::Value::Null,
        )?;
        Ok(self.status().await)
    }
    pub async fn resume(self: &Arc<Self>) -> Result<ServiceStatus> {
        self.resume_owned(false).await
    }
    pub async fn resume_after_wake(self: &Arc<Self>) -> Result<ServiceStatus> {
        self.resume_owned(true).await
    }
    async fn resume_owned(self: &Arc<Self>, wake: bool) -> Result<ServiceStatus> {
        let engine = self.clone();
        tokio::spawn(async move {
            let _transition = engine.lifecycle.lock().await;
            let _reservation = engine.submissions.lock().await;
            let mut state = engine.state.lock().await;
            if state.stopping {
                return Err(stopping());
            }
            // Waking preserves manual pause as well as lost-session and storage
            // quarantine. A manual resume during sleep cannot admit new feeds.
            engine.database.event(
                if wake {
                    "service.wake"
                } else {
                    "service.resume"
                },
                &serde_json::Value::Null,
            )?;
            if wake {
                state.sleeping = false;
            } else {
                state.manual_pause = false;
            }
            state.paused = state.manual_pause || state.sleeping;
            drop(state);
            engine.wake.notify_one();
            Ok(engine.status().await)
        })
        .await
        .map_err(crate::image_access::worker)?
    }
    async fn wait_idle(&self) {
        loop {
            let changed = self.idle.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let state = self.state.lock().await;
            if state.active.is_none() && !state.inspecting && !state.camera {
                return;
            }
            drop(state);
            changed.await;
        }
    }
}
fn stopping() -> ScannerError {
    ScannerError::new("busy", "Service is shutting down.")
}
