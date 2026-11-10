use crate::{Engine, settings_support};
use scanner_types::{
    error::{Result, ScannerError},
    service::{DeviceStatus, ScanSettings},
};
use std::sync::Arc;

impl Engine {
    pub async fn configured_settings(&self, id: String) -> Result<Option<ScanSettings>> {
        let database = self.database.clone();
        tokio::task::spawn_blocking(move || {
            database
                .device_settings(&id)?
                .map(serde_json::from_value)
                .transpose()
                .map_err(storage)
        })
        .await
        .map_err(storage)?
    }
    /// Save defaults after a read-only capability inspection; never feed or write
    /// transport settings. Capture still verifies effective settings before feeding.
    pub async fn set_settings(
        self: &Arc<Self>,
        device_id: Option<String>,
        settings: ScanSettings,
    ) -> Result<DeviceStatus> {
        settings.validate()?;
        let engine = self.clone();
        tokio::spawn(async move {
            let _reservation = engine.submissions.lock().await;
            let report = engine.inspect(device_id).await?;
            if !report.errors.is_empty() || !report.session_closed {
                return Err(ScannerError::new("inspection", "Could not inspect and close scanner session; defaults were not changed."));
            }
            settings_support::validate(&report, &settings)?;
            {
                let state = engine.state.lock().await;
                state.accepting()?;
                if state.active.is_some() || !state.queue.is_empty() {
                    return Err(ScannerError::new("busy", "Service state changed; defaults were not changed."));
                }
            }
            let database = engine.database.clone();
            let id = report.device.id.clone();
            let encoded = serde_json::to_value(&settings).map_err(storage)?;
            tokio::task::spawn_blocking(move || database.save_device_settings(&id, &encoded)).await.map_err(storage)??;
            Ok(DeviceStatus {
                schema_version:1, device:report.device.clone(), service:engine.status().await,
                inspection:Some(report), configured_settings:Some(settings), ready:None,
                warnings:vec!["Defaults saved for future jobs using useDeviceSettings=true. No hardware settings were written or scan initiated.".into(),
                "Current capture uses 600 DPI A4 RGB 8-bit; effective settings are verified before feeding.".into()],
            })
        }).await.map_err(storage)?
    }
    pub async fn device_status(
        self: &Arc<Self>,
        device_id: Option<String>,
    ) -> Result<DeviceStatus> {
        let device = self.select_device(device_id.as_deref()).await?;
        let mut warnings = vec![];
        let inspection = match self.inspect(Some(device.id.clone())).await {
            Ok(report) => Some(report),
            Err(e) if e.kind == "busy" => {
                warnings.push(e.message);
                None
            }
            Err(e) => return Err(e),
        };
        let service = self.status().await;
        let ready = if service.active_job_id.is_some()
            || service.queued_jobs > 0
            || service.paused
            || service.stopping
            || service.requires_restart
            || service.persistence_error.is_some()
        {
            Some(false)
        } else {
            inspection.as_ref().and_then(|r| {
                if !r.errors.is_empty() || !r.session_closed {
                    Some(false)
                } else {
                    settings_support::loaded(r)
                }
            })
        };
        let configured_settings = self.configured_settings(device.id.clone()).await?;
        warnings.push(
            "Readiness is a momentary ADF observation, not a guarantee a later scan will succeed."
                .into(),
        );
        Ok(DeviceStatus {
            schema_version: 1,
            device,
            service,
            inspection,
            configured_settings,
            ready,
            warnings,
        })
    }
}
fn storage(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("storage", error.to_string())
}
