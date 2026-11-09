use crate::Engine;
use scanner_store::Database;
use scanner_types::{
    error::{Result, ScannerError},
    service::{ProfilePatch, ScanProfile},
};
use std::sync::Arc;

impl Engine {
    pub async fn profiles(&self) -> Result<Vec<ScanProfile>> {
        let database = self.database.clone();
        tokio::task::spawn_blocking(move || {
            database
                .profiles()?
                .into_iter()
                .map(|value| serde_json::from_value(value).map_err(storage))
                .collect()
        })
        .await
        .map_err(storage)?
    }
    pub async fn profile(&self, identifier: String) -> Result<ScanProfile> {
        let database = self.database.clone();
        tokio::task::spawn_blocking(move || database.find_profile(&identifier))
            .await
            .map_err(storage)?
    }
    pub async fn create_profile(self: &Arc<Self>, profile: ScanProfile) -> Result<ScanProfile> {
        self.change_profile(move |database| database.create_profile(&profile))
            .await
    }
    pub async fn save_profile(self: &Arc<Self>, profile: ScanProfile) -> Result<ScanProfile> {
        self.change_profile(move |database| {
            profile.validate()?;
            database.save_profile(
                profile.id,
                &serde_json::to_value(&profile).map_err(storage)?,
            )?;
            Ok(profile)
        })
        .await
    }
    pub async fn update_profile(
        self: &Arc<Self>,
        identifier: String,
        patch: ProfilePatch,
    ) -> Result<ScanProfile> {
        self.change_profile(move |database| database.update_profile(&identifier, &patch))
            .await
    }
    pub async fn remove_profile(self: &Arc<Self>, identifier: String) -> Result<()> {
        self.change_profile(move |database| database.remove_profile(&identifier))
            .await
    }
    async fn change_profile<T: Send + 'static>(
        self: &Arc<Self>,
        change: impl FnOnce(&Database) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let engine = self.clone();
        // An accepted profile mutation outlives its API caller, and shutdown
        // waits for this reservation before completing the service worker.
        tokio::spawn(async move {
            let _reservation = engine.submissions.lock().await;
            if engine.state.lock().await.stopping {
                return Err(ScannerError::new("busy", "Service is shutting down."));
            }
            let database = engine.database.clone();
            tokio::task::spawn_blocking(move || change(&database))
                .await
                .map_err(storage)?
        })
        .await
        .map_err(storage)?
    }
}
fn storage(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("storage", error.to_string())
}
