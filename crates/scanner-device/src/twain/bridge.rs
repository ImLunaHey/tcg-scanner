use super::{
    inspection,
    manager::{Manager, error},
    window::Window,
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
        atomic::{AtomicBool, Ordering},
        mpsc::{self as sync, SyncSender},
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

static IN_USE: AtomicBool = AtomicBool::new(false);
struct Lease;
impl Drop for Lease {
    fn drop(&mut self) {
        IN_USE.store(false, Ordering::SeqCst);
    }
}
enum Command {
    Devices(oneshot::Sender<Result<Vec<Device>>>),
    Inspect(String, oneshot::Sender<Result<Inspection>>),
}
/// Windows TWAIN 2 discovery and read-only inspection. Owns a hidden window and
/// DSM on one worker; no helper executable or driver UI is launched.
#[derive(Clone)]
pub struct TwainBackend {
    commands: SyncSender<Command>,
}
impl TwainBackend {
    /// `library` must identify a trusted TWAIN 2 DSM with the process architecture.
    pub async fn new(library: PathBuf) -> Result<Self> {
        if !library.is_absolute() {
            return Err(error("Select an absolute trusted TWAIN DSM path."));
        }
        IN_USE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| ScannerError::new("busy", "A TWAIN adapter already owns this process."))?;
        let lease = Lease;
        let (commands, receiver) = sync::sync_channel(16);
        let (ready, result) = oneshot::channel();
        std::thread::Builder::new()
            .name("scanner-twain".into())
            .spawn(move || {
                let _lease = lease;
                let window = match Window::new() {
                    Ok(window) => window,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                // SAFETY: window lives on this worker and outlives the manager.
                let mut manager = match unsafe { Manager::load(&library, window.0) } {
                    Ok(manager) => manager,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                let initialized = manager.initialize();
                let successful = initialized.is_ok();
                if ready.send(initialized).is_ok() && successful {
                    loop {
                        window.pump();
                        match receiver.recv_timeout(Duration::from_millis(20)) {
                            Ok(Command::Devices(reply)) => {
                                let result = manager.identities().map(|identities| {
                                    identities
                                        .into_iter()
                                        .map(|identity| manager.device(identity))
                                        .collect()
                                });
                                let _ = reply.send(result);
                            }
                            Ok(Command::Inspect(id, reply)) => {
                                let _ = reply.send(inspection::inspect(&mut manager, &id));
                            }
                            Err(sync::RecvTimeoutError::Timeout) => {}
                            Err(sync::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                }
                if manager.shutdown().is_err() {
                    // A failed native close cannot be repaired by dropping the DLL,
                    // window or owner thread. Keep all three until process restart.
                    loop {
                        window.pump();
                        std::thread::park_timeout(Duration::from_secs(1));
                    }
                }
            })
            .map_err(error)?;
        result.await.map_err(error)??;
        Ok(Self { commands })
    }
    fn send(&self, command: Command) -> Result<()> {
        self.commands
            .try_send(command)
            .map_err(|failure| match failure {
                sync::TrySendError::Full(_) => {
                    ScannerError::new("busy", "TWAIN command queue is full.")
                }
                sync::TrySendError::Disconnected(_) => {
                    ScannerError::new("disconnected", "TWAIN worker stopped.")
                }
            })
    }
}
#[async_trait]
impl ScannerBackend for TwainBackend {
    async fn devices(&self) -> Result<Vec<Device>> {
        let (reply, result) = oneshot::channel();
        self.send(Command::Devices(reply))?;
        result.await.map_err(error)?
    }
    async fn inspect(&self, id: &str) -> Result<Inspection> {
        let (reply, result) = oneshot::channel();
        self.send(Command::Inspect(id.into(), reply))?;
        result.await.map_err(error)?
    }
    async fn capture(
        &self,
        _id: &str,
        _request: &ScanRequest,
        _incoming: PathBuf,
        _events: mpsc::Sender<CaptureEvent>,
        _cancel: CancellationToken,
    ) -> Result<CaptureOutcome> {
        Err(ScannerError::new(
            "unsupported",
            "TWAIN acquisition is not available yet.",
        ))
    }
}
