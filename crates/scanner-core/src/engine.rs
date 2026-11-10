use crate::{
    backend::{CardProcessor, ScannerBackend},
    runner::Work,
};
use scanner_files::StorageRoot;
use scanner_store::Database;
use scanner_types::{
    device::Device,
    error::{Result, ScannerError},
    scan::ScanRecord,
    service::ServiceStatus,
};
use std::{collections::VecDeque, sync::Arc};
use tokio::sync::{Mutex, Notify, Semaphore, watch};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct Engine {
    pub(crate) backend: Arc<dyn ScannerBackend>,
    pub(crate) processor: Arc<dyn CardProcessor>,
    pub(crate) database: Arc<Database>,
    pub(crate) root: Arc<StorageRoot>,
    pub(crate) state: Mutex<State>,
    pub(crate) submissions: Mutex<()>,
    pub(crate) wake: Notify,
    pub(crate) event_waiters: Semaphore,
    pub(crate) thumbnail_workers: Arc<Semaphore>,
    pub(crate) edit_workers: Arc<Semaphore>,
    pub(crate) done: watch::Sender<bool>,
}

#[derive(Default)]
pub(crate) struct State {
    pub queue: VecDeque<Work>,
    pub active: Option<(Uuid, CancellationToken)>,
    pub inspecting: bool,
    pub paused: bool,
    pub stopping: bool,
    pub quarantined: bool,
    pub persistence_error: Option<ScannerError>,
}

impl State {
    pub fn accepting(&self) -> Result<()> {
        if self.stopping || self.paused || self.quarantined || self.persistence_error.is_some() {
            return Err(ScannerError::new(
                "busy",
                "Service is paused, stopping or requires recovery.",
            ));
        }
        if self.queue.len() >= 100 {
            return Err(ScannerError::new(
                "resourceLimit",
                "Queue is full (100 waiting jobs).",
            ));
        }
        Ok(())
    }
}

impl Engine {
    pub fn new(
        backend: Arc<dyn ScannerBackend>,
        processor: Arc<dyn CardProcessor>,
        database: Arc<Database>,
        root: Arc<StorageRoot>,
    ) -> Arc<Self> {
        let (done, _) = watch::channel(false);
        let engine = Arc::new(Self {
            backend,
            processor,
            database,
            root,
            state: Mutex::new(State::default()),
            submissions: Mutex::new(()),
            wake: Notify::new(),
            event_waiters: Semaphore::new(8),
            thumbnail_workers: Arc::new(Semaphore::new(2)),
            edit_workers: Arc::new(Semaphore::new(2)),
            done,
        });
        tokio::spawn(engine.clone().run());
        engine
    }
    pub fn database(&self) -> &Database {
        &self.database
    }
    pub fn storage(&self) -> &StorageRoot {
        &self.root
    }
    pub async fn status(&self) -> ServiceStatus {
        let state = self.state.lock().await;
        ServiceStatus {
            schema_version: 1,
            active_job_id: state.active.as_ref().map(|(id, _)| *id),
            queued_jobs: state.queue.len(),
            paused: state.paused,
            stopping: state.stopping,
            persistence_error: state.persistence_error.clone(),
            storage_root: self.root.path().to_string_lossy().into(),
            requires_restart: state.quarantined,
        }
    }
    pub fn scan_status(&self, id: Uuid) -> Result<ScanRecord> {
        self.database
            .job(id)?
            .ok_or_else(|| ScannerError::new("scanNotFound", "Unknown scan ID."))
    }
    pub async fn devices(&self) -> Result<Vec<Device>> {
        let devices = self.backend.devices().await?;
        self.database.observe_devices(&devices)?;
        Ok(devices)
    }
    pub async fn cancel_scan(&self, id: Uuid) -> Result<ScanRecord> {
        let mut state = self.state.lock().await;
        if let Some((active, cancel)) = &state.active
            && *active == id
        {
            cancel.cancel();
        }
        let queued = state
            .queue
            .iter()
            .position(|work| work.id() == id)
            .and_then(|i| state.queue.remove(i));
        drop(state);
        if let Some(mut work) = queued {
            self.finish_queued(&mut work, true, None).await?;
        }
        self.scan_status(id)
    }
    pub async fn shutdown(&self) {
        let mut done = self.done.subscribe();
        {
            // Finish any accepted submission before allowing the worker to exit.
            let _reservation = self.submissions.lock().await;
            let mut state = self.state.lock().await;
            state.stopping = true;
            if let Some((_, cancel)) = &state.active {
                cancel.cancel();
            }
        }
        self.wake.notify_one();
        let _ = done.wait_for(|done| *done).await;
    }
    pub(crate) async fn select_device(&self, id: Option<&str>) -> Result<Device> {
        let devices = self.backend.devices().await?;
        match id {
            Some(id) => devices
                .into_iter()
                .find(|device| device.id == id)
                .ok_or_else(|| {
                    ScannerError::new("deviceNotFound", "No scanner matches the requested ID.")
                }),
            None if devices.len() == 1 => Ok(devices.into_iter().next().unwrap()),
            None if devices.is_empty() => Err(ScannerError::new(
                "deviceNotFound",
                "No scanner found. Check its connection, power and driver.",
            )),
            None => Err(ScannerError::new(
                "ambiguousDevice",
                "Multiple scanners found; select a device ID.",
            )),
        }
    }
}
