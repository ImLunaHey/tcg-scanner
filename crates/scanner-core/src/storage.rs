use crate::Engine;
use scanner_types::{
    error::{Result, ScannerError},
    service::StorageStatus,
};
use std::sync::Arc;

impl Engine {
    pub async fn storage_status(&self) -> Result<StorageStatus> {
        let permit = self
            .inventory_workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| ScannerError::new("resourceLimit", "Storage reads are busy."))?;
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            root.status()
        })
        .await
        .map_err(crate::image_access::worker)
    }
    /// Revalidate the configured destination and replace its pinned capability.
    /// Never mounts a share or writes files. All accepted mutations drain first.
    pub async fn reconnect_storage(self: &Arc<Self>) -> Result<StorageStatus> {
        let engine = self.clone();
        tokio::spawn(async move {
            let _reservation = engine.submissions.lock().await;
            {
                let mut state = engine.state.lock().await;
                if state.stopping
                    || state.active.is_some()
                    || !state.queue.is_empty()
                    || state.inspecting
                    || state.camera
                    || state.button.is_some()
                    || engine.transfer.borrow().is_some()
                {
                    return Err(ScannerError::new(
                        "busy",
                        "Reconnect storage only while the service is idle.",
                    ));
                }
                state.inspecting = true;
                state.button_epoch = state.button_epoch.wrapping_add(1);
            }
            let root = engine.root.clone();
            let result = tokio::task::spawn_blocking(move || {
                root.reconnect()?;
                Ok(root.status())
            })
            .await
            .map_err(crate::image_access::worker)
            .and_then(|v| v);
            engine.state.lock().await.inspecting = false;
            engine.wake.notify_one();
            engine.idle.notify_waiters();
            result
        })
        .await
        .map_err(crate::image_access::worker)?
    }
}
