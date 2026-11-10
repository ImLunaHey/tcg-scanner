use crate::{
    Engine,
    backend::{CaptureEvent, CaptureOutcome},
};
use scanner_files::BatchStore;
use scanner_types::{
    error::{Result, ScannerError},
    scan::ScanState,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(crate) struct Work {
    pub store: BatchStore,
    initial_images: usize,
}
impl Work {
    pub fn new(store: BatchStore) -> Self {
        let initial_images = store.record().images.len();
        Self {
            store,
            initial_images,
        }
    }
    pub fn id(&self) -> Uuid {
        self.store.record().id
    }
}
enum Next {
    Capture(Work, CancellationToken),
    Cancel(Work, Option<ScannerError>),
    Wait,
    Stop,
}

impl Engine {
    pub(crate) async fn run(self: Arc<Self>) {
        loop {
            let notified = self.wake.notified();
            match self.next().await {
                Next::Capture(mut work, cancel) => {
                    self.execute(&mut work, cancel).await;
                    self.state.lock().await.active = None;
                }
                Next::Cancel(mut work, error) => {
                    let cancelled = error.is_none();
                    let _ = self.finish_queued(&mut work, cancelled, error).await;
                }
                Next::Wait => notified.await,
                Next::Stop => break,
            }
        }
        self.done.send_replace(true);
    }
    async fn next(&self) -> Next {
        let mut state = self.state.lock().await;
        if state.stopping || state.quarantined || state.persistence_error.is_some() {
            if let Some(work) = state.queue.pop_front() {
                let error = if state.stopping {
                    None
                } else {
                    Some(ScannerError::new(
                        "disconnected",
                        "Queued capture cancelled because scanner or storage recovery is required.",
                    ))
                };
                return Next::Cancel(work, error);
            }
            return if state.stopping && !state.inspecting && !state.camera {
                Next::Stop
            } else {
                Next::Wait
            };
        }
        if state.paused || state.inspecting || state.camera {
            return Next::Wait;
        }
        if let Some(work) = state.queue.pop_front() {
            let cancel = CancellationToken::new();
            state.active = Some((work.id(), cancel.clone()));
            Next::Capture(work, cancel)
        } else {
            Next::Wait
        }
    }
    pub(crate) async fn finish_queued(
        &self,
        work: &mut Work,
        cancelled: bool,
        error: Option<ScannerError>,
    ) -> Result<()> {
        let result = work
            .store
            .finish(error.into_iter().collect(), cancelled, true)
            .and_then(|()| self.database.save(work.store.record()));
        if let Err(error) = &result {
            self.state.lock().await.persistence_error = Some(error.clone());
        }
        result
    }
    async fn execute(&self, work: &mut Work, cancel: CancellationToken) {
        if let Err(error) = self.transition(work, |record| {
            record.state = ScanState::Preparing;
            if record.started_at.is_none() {
                record.started_at = Some(chrono::Utc::now());
            }
        }) {
            self.state.lock().await.persistence_error = Some(error.clone());
            let _ = self.finish_queued(work, false, Some(error)).await;
            return;
        }
        let mut outcome = if cancel.is_cancelled() {
            CaptureOutcome {
                errors: vec![],
                cancelled: true,
                session_closed: true,
            }
        } else {
            self.capture(work, cancel.clone()).await
        };
        if !outcome.session_closed {
            self.state.lock().await.quarantined = true;
            outcome.errors.push(ScannerError::new(
                "closeSession",
                "Scanner session closure was not confirmed.",
            ));
        }
        if outcome.errors.iter().any(|error| {
            matches!(
                error.kind.as_str(),
                "disconnected" | "deviceNotFound" | "cleanup" | "closeSession"
            )
        }) {
            self.state.lock().await.quarantined = true;
        }
        let result = async {
            self.transition(work, |record| record.state = ScanState::Processing)?;
            if outcome.errors.is_empty()
                && !outcome.cancelled
                && work.store.record().request.prepare_cards == Some(true)
            {
                self.prepare(work, &cancel).await?;
            }
            work.store.finish(
                outcome.errors,
                outcome.cancelled || cancel.is_cancelled(),
                outcome.session_closed,
            )?;
            self.database.save(work.store.record())
        }
        .await;
        if let Err(error) = result {
            self.state.lock().await.persistence_error = Some(error.clone());
            let mut record = work.store.record().clone();
            record.errors.push(error);
            record.state = ScanState::Failed;
            record.completed_at = Some(chrono::Utc::now());
            let _ = self.database.save(&record);
        }
    }
    pub(crate) fn transition(
        &self,
        work: &mut Work,
        change: impl FnOnce(&mut scanner_types::scan::ScanRecord),
    ) -> Result<()> {
        work.store.update(change)?;
        self.database.save(work.store.record())
    }
    async fn prepare(&self, work: &mut Work, cancel: &CancellationToken) -> Result<()> {
        let sequences: Vec<_> = work
            .store
            .record()
            .images
            .iter()
            .skip(work.initial_images)
            .map(|image| image.sequence)
            .collect();
        for &sequence in &sequences {
            if cancel.is_cancelled() {
                break;
            }
            let result = self.processor.prepare(work.store.record(), sequence).await;
            self.transition(work, |record| match result {
                Ok(edit) => record
                    .processed_images
                    .get_or_insert_with(Vec::new)
                    .push(edit),
                Err(error) => record
                    .warnings
                    .push(format!("Image {sequence}: {}", error.message)),
            })?;
        }
        self.identify_captured(work, &sequences, cancel).await
    }
    async fn capture(&self, work: &mut Work, cancel: CancellationToken) -> CaptureOutcome {
        let (tx, mut rx) = mpsc::channel(8);
        let backend = self.backend.clone();
        let request = work.store.record().request.clone();
        let device = work.store.record().device.id.clone();
        let incoming = work.store.incoming_path().to_path_buf();
        let token = cancel.clone();
        let mut task = tokio::spawn(async move {
            backend
                .capture(&device, &request, incoming, tx, token)
                .await
        });
        let mut errors = vec![];
        let mut receiving = true;
        let mut next_sequence = 1;
        let mut outcome = loop {
            tokio::select! {
                result=&mut task=>{
                    rx.close();
                    while let Some(event)=rx.recv().await {self.capture_event(work,event,&mut next_sequence,&mut errors,&cancel).await;}
                    break match result {
                        Ok(Ok(outcome))=>outcome,
                        Ok(Err(error))=>CaptureOutcome{errors:vec![error],cancelled:cancel.is_cancelled(),session_closed:false},
                        Err(error)=>CaptureOutcome{errors:vec![ScannerError::new("capture",error.to_string())],cancelled:false,session_closed:false},
                    };
                }
                event=rx.recv(),if receiving=>match event {
                    Some(event)=>self.capture_event(work,event,&mut next_sequence,&mut errors,&cancel).await,
                    None=>receiving=false,
                }
            }
        };
        outcome.errors.extend(errors);
        outcome
    }
    async fn capture_event(
        &self,
        work: &mut Work,
        event: CaptureEvent,
        next: &mut u32,
        errors: &mut Vec<ScannerError>,
        cancel: &CancellationToken,
    ) {
        if !errors.is_empty() {
            return;
        }
        let result = match event {
            CaptureEvent::Settings {
                settings,
                acknowledged,
            } => {
                let result = self.transition(work, |record| {
                    record.state = ScanState::Scanning;
                    record.applied_settings = settings;
                });
                let _ = acknowledged.send(result.clone());
                result
            }
            CaptureEvent::Image { filename, sequence } => {
                if sequence != *next {
                    Err(ScannerError::new(
                        "captureOrder",
                        "Driver delivered captures out of acquisition order; staging retained.",
                    ))
                } else {
                    *next += 1;
                    work.store
                        .preserve(&filename, sequence)
                        .and_then(|_| self.database.save(work.store.record()))
                }
            }
        };
        if let Err(error) = result {
            cancel.cancel();
            self.state.lock().await.persistence_error = Some(error.clone());
            errors.push(error);
        }
    }
}
