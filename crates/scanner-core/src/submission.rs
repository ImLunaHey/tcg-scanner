use crate::{Engine, runner::Work};
use scanner_files::BatchStore;
use scanner_types::{
    error::{Result, ScannerError},
    scan::{ScanBackground, ScanRecord, ScanRequest},
    service::{ScanProfile, ScanSettings},
};
use std::sync::Arc;
use uuid::Uuid;

impl Engine {
    pub async fn start_scan(self: &Arc<Self>, submission: ScanRequest) -> Result<ScanRecord> {
        let engine = self.clone();
        // A disconnected API caller must not strand a reserved batch outside the queue.
        tokio::spawn(async move { engine.submit_start(submission).await })
            .await
            .map_err(|e| ScannerError::new("storage", e.to_string()))?
    }
    async fn submit_start(&self, submission: ScanRequest) -> Result<ScanRecord> {
        submission.validate()?;
        let _reservation = self.submissions.lock().await;
        if let Some(existing) = self.existing(&submission)? {
            return Ok(existing);
        }
        self.state.lock().await.accepting()?;
        let device = self.select_device(submission.device_id.as_deref()).await?;
        let mut request = submission.clone();
        let profile = self.resolve_settings(&device.id, &mut request)?;
        self.state.lock().await.accepting()?;
        let (root, database) = (self.root.clone(), self.database.clone());
        let mut store = tokio::task::spawn_blocking(move || -> Result<BatchStore> {
            let number = database.reserve_batch_number(request.request_id)?;
            let mut store = BatchStore::create(&root, device, request, number)?;
            if let Some(profile) = profile {
                store.update(|record| {
                    record.extensions.insert("profile".into(), profile);
                })?;
            }
            database.insert(store.record(), &submission)?;
            Ok(store)
        })
        .await
        .map_err(|e| ScannerError::new("storage", e.to_string()))??;
        let record = store.record().clone();
        let mut state = self.state.lock().await;
        if state.stopping {
            drop(state);
            store.finish(vec![], true, true)?;
            self.database.save(store.record())?;
            return Ok(store.record().clone());
        }
        state.queue.push_back(Work::new(store));
        drop(state);
        self.wake.notify_one();
        Ok(record)
    }
    pub async fn continue_batch(
        self: &Arc<Self>,
        id: Uuid,
        request_id: Uuid,
    ) -> Result<ScanRecord> {
        let engine = self.clone();
        tokio::spawn(async move { engine.submit_continuation(id, request_id).await })
            .await
            .map_err(|e| ScannerError::new("storage", e.to_string()))?
    }
    async fn submit_continuation(&self, id: Uuid, request_id: Uuid) -> Result<ScanRecord> {
        let _reservation = self.submissions.lock().await;
        if let Some(batch) = self.database.continuation_batch(request_id)? {
            if batch != id {
                return Err(ScannerError::new(
                    "conflict",
                    "Continuation request belongs to another batch.",
                ));
            }
            return self.scan_status(id);
        }
        if self.database.submission(request_id)?.is_some() {
            return Err(ScannerError::new(
                "conflict",
                "Request ID belongs to an original scan.",
            ));
        }
        self.state.lock().await.accepting()?;
        let previous = self.scan_status(id)?;
        self.select_device(Some(&previous.device.id)).await?;
        let (root, database) = (self.root.clone(), self.database.clone());
        let store = tokio::task::spawn_blocking(move || -> Result<BatchStore> {
            let store = BatchStore::continuing(&root, &previous, request_id)?;
            database.begin_continuation(store.record(), request_id)?;
            if let Err(error) = store.publish() {
                let mut failed = store.record().clone();
                failed.state = scanner_types::scan::ScanState::Failed;
                failed.errors.push(error.clone());
                failed.completed_at = Some(chrono::Utc::now());
                failed.session_closed = true; // No driver session has started.
                database.save(&failed)?;
                return Err(error);
            }
            Ok(store)
        })
        .await
        .map_err(|e| ScannerError::new("storage", e.to_string()))??;
        let record = store.record().clone();
        self.state.lock().await.queue.push_back(Work::new(store));
        self.wake.notify_one();
        Ok(record)
    }
    fn existing(&self, request: &ScanRequest) -> Result<Option<ScanRecord>> {
        if self
            .database
            .continuation_batch(request.request_id)?
            .is_some()
        {
            return Err(ScannerError::new(
                "conflict",
                "Request ID belongs to a continuation.",
            ));
        }
        let Some(previous) = self.database.submission(request.request_id)? else {
            return Ok(None);
        };
        if previous != *request {
            return Err(ScannerError::new(
                "conflict",
                "Request ID was already used with different settings.",
            ));
        }
        Ok(Some(self.scan_status(request.request_id)?))
    }
    fn resolve_settings(
        &self,
        device: &str,
        request: &mut ScanRequest,
    ) -> Result<Option<serde_json::Value>> {
        if request.overscan.is_none()
            && request.prepare_cards == Some(true)
            && request.background != Some(ScanBackground::White)
        {
            request.overscan = Some(true);
        }
        let profile = request
            .profile_id
            .map(|id| {
                self.database
                    .profile(id)?
                    .ok_or_else(|| ScannerError::new("profileNotFound", "Unknown scan profile."))
            })
            .transpose()?;
        if let Some(profile) = &profile {
            let profile: ScanProfile = serde_json::from_value(profile.clone())
                .map_err(|e| ScannerError::new("storage", e.to_string()))?;
            profile.validate()?;
            profile.settings.apply(request);
        } else if request.use_device_settings == Some(true) {
            let settings = self.database.device_settings(device)?.ok_or_else(|| {
                ScannerError::new(
                    "settingsNotFound",
                    "No configured defaults for this scanner.",
                )
            })?;
            let settings: ScanSettings = serde_json::from_value(settings)
                .map_err(|e| ScannerError::new("storage", e.to_string()))?;
            settings.validate()?;
            settings.apply(request);
        }
        request.dpi = 600;
        request.validate()?;
        Ok(profile)
    }
}
