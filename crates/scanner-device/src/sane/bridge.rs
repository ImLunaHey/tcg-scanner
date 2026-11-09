use super::{
    cancellation::Cancellation,
    discovery,
    ffi::{self, Api},
    session,
};
use crate::{CaptureEvent, CaptureOutcome, ScannerBackend};
use async_trait::async_trait;
use scanner_types::{
    device::{Device, Inspection},
    error::{Result, ScannerError},
    scan::ScanRequest,
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self as sync, SyncSender},
    },
    time::Duration,
};
use tokio::{
    runtime::Handle,
    sync::{mpsc, oneshot},
};
use tokio_util::sync::CancellationToken;

static IN_USE: AtomicBool = AtomicBool::new(false);
struct Lease;
impl Drop for Lease {
    fn drop(&mut self) {
        IN_USE.store(false, Ordering::SeqCst);
    }
}
/// In-process SANE 1 adapter. One worker owns the library and every native handle.
/// Only `sane_cancel`, explicitly asynchronous in SANE, crosses that boundary.
#[derive(Clone)]
pub struct SaneBackend {
    commands: SyncSender<Command>,
}
struct CaptureCommand {
    id: String,
    request: ScanRequest,
    incoming: PathBuf,
    events: mpsc::Sender<CaptureEvent>,
    cancellation: Arc<Cancellation>,
    reply: oneshot::Sender<CaptureOutcome>,
}
enum Command {
    Devices(oneshot::Sender<Result<Vec<Device>>>),
    Inspect(String, oneshot::Sender<Result<Inspection>>),
    Capture(Box<CaptureCommand>),
}
impl SaneBackend {
    /// Load a trusted, application-selected SANE library. No executable is used.
    /// A running Tokio runtime is required. The adapter owns SANE process-wide;
    /// opening another adapter fails instead of racing global init/exit calls.
    pub async fn new(library: PathBuf) -> Result<Self> {
        IN_USE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| ScannerError::new("busy", "A SANE adapter already owns this process."))?;
        let lease = Lease;
        let runtime = Handle::current();
        let (commands, receiver) = sync::sync_channel(16);
        let (ready, result) = oneshot::channel();
        std::thread::Builder::new()
            .name("scanner-sane".into())
            .spawn(move || {
                let _lease = lease;
                let api = match Api::load(&library) {
                    Ok(api) => api,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                if ready.send(Ok(())).is_err() {
                    return;
                }
                while let Ok(command) = receiver.recv() {
                    match command {
                        Command::Devices(reply) => {
                            let _ = reply.send(discovery::devices(&api, &library));
                        }
                        Command::Inspect(id, reply) => {
                            let result = discovery::devices(&api, &library).and_then(|devices| {
                                let device = devices
                                    .into_iter()
                                    .find(|device| device.id == id)
                                    .ok_or_else(|| {
                                        ScannerError::new(
                                            "deviceNotFound",
                                            "SANE device not found.",
                                        )
                                    })?;
                                discovery::inspect(&api, device)
                            });
                            let _ = reply.send(result);
                        }
                        Command::Capture(command) => {
                            let result = session::capture(
                                &api,
                                session::Capture {
                                    id: &command.id,
                                    request: &command.request,
                                    incoming: &command.incoming,
                                    events: &command.events,
                                    cancellation: command.cancellation,
                                    runtime: &runtime,
                                },
                            );
                            let _ = command.reply.send(result);
                        }
                    }
                }
            })
            .map_err(ffi::error)?;
        result.await.map_err(closed)??;
        Ok(Self { commands })
    }
    fn send(&self, command: Command) -> Result<()> {
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                sync::TrySendError::Full(_) => {
                    ScannerError::new("busy", "SANE command queue is full.")
                }
                sync::TrySendError::Disconnected(_) => closed("SANE worker stopped."),
            })
    }
}
#[async_trait]
impl ScannerBackend for SaneBackend {
    async fn devices(&self) -> Result<Vec<Device>> {
        let (reply, result) = oneshot::channel();
        self.send(Command::Devices(reply))?;
        result.await.map_err(closed)?
    }
    async fn inspect(&self, id: &str) -> Result<Inspection> {
        let (reply, result) = oneshot::channel();
        self.send(Command::Inspect(id.into(), reply))?;
        result.await.map_err(closed)?
    }
    async fn capture(
        &self,
        id: &str,
        request: &ScanRequest,
        incoming: PathBuf,
        events: mpsc::Sender<CaptureEvent>,
        cancel: CancellationToken,
    ) -> Result<CaptureOutcome> {
        let cancellation = Arc::new(Cancellation::default());
        cancellation.poll(cancel.is_cancelled());
        let (reply, mut result) = oneshot::channel();
        self.send(Command::Capture(Box::new(CaptureCommand {
            id: id.into(),
            request: request.clone(),
            incoming,
            events,
            cancellation: cancellation.clone(),
            reply,
        })))?;
        // Ownership survives the caller dropping its future. The worker finishes
        // cleanup and this supervisor keeps issuing cancellation until it does.
        tokio::spawn(async move {
            let mut timer=tokio::time::interval(Duration::from_millis(50));
            loop {
                tokio::select! {
                    outcome=&mut result=>return outcome.map_err(closed),
                    _=timer.tick()=>{
                        let cancellation=cancellation.clone();let cancelled=cancel.is_cancelled();
                        tokio::task::spawn_blocking(move || cancellation.poll(cancelled)).await.map_err(closed)?;
                    }
                }
            }
        }).await.map_err(closed)?
    }
}
fn closed(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("disconnected", error.to_string())
}
