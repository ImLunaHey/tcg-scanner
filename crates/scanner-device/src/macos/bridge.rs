use super::browser::Controller;
use crate::{CaptureEvent, CaptureOutcome, ScannerBackend};
use async_trait::async_trait;
use futures::{executor::LocalPool, task::LocalSpawnExt};
use objc2::{MainThreadMarker, rc::autoreleasepool};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSRunLoop};
use scanner_types::{
    device::{Device, Inspection},
    error::{Result, ScannerError},
    scan::ScanRequest,
};
use std::{cell::Cell, path::PathBuf, rc::Rc, sync::mpsc};
use tokio::sync::{mpsc as events, oneshot};
use tokio_util::sync::CancellationToken;

enum Command {
    Devices(oneshot::Sender<Result<Vec<Device>>>),
    Inspect(String, oneshot::Sender<Result<Inspection>>),
    Capture {
        device: String,
        request: ScanRequest,
        incoming: PathBuf,
        events: events::Sender<CaptureEvent>,
        cancel: CancellationToken,
        reply: oneshot::Sender<Result<CaptureOutcome>>,
    },
    Recover(oneshot::Sender<bool>),
    Button {
        device: String,
        seconds: Option<u32>,
        cancel: CancellationToken,
        reply: oneshot::Sender<Result<()>>,
    },
}
/// Sendable command handle. Every Objective-C object stays on the main thread.
#[derive(Clone)]
pub struct MacBackend {
    sender: mpsc::SyncSender<Command>,
}
/// Own on the process main thread and call `tick` while the service runs. The
/// supplied runtime must keep its timer driver alive until all sessions close.
pub struct MacHost {
    commands: mpsc::Receiver<Command>,
    controller: Rc<Controller>,
    executor: LocalPool,
    runtime: tokio::runtime::Handle,
    _main_thread: MainThreadMarker,
    pending: Rc<Cell<usize>>,
}
impl MacHost {
    pub fn new(runtime: tokio::runtime::Handle) -> Result<(Self, MacBackend)> {
        let marker = MainThreadMarker::new().ok_or_else(|| {
            ScannerError::new(
                "driver",
                "ImageCaptureCore must be hosted on the main thread.",
            )
        })?;
        let controller = Rc::new(Controller::new(marker));
        let (sender, commands) = mpsc::sync_channel(128);
        Ok((
            Self {
                commands,
                controller,
                executor: LocalPool::new(),
                runtime,
                _main_thread: marker,
                pending: Rc::new(Cell::new(0)),
            },
            MacBackend { sender },
        ))
    }
    pub fn tick(&mut self) {
        let _runtime = self.runtime.enter();
        autoreleasepool(|_| {
            for _ in 0..128 {
                let Ok(command) = self.commands.try_recv() else {
                    break;
                };
                if self.pending.get() >= 128 {
                    command.reject();
                    continue;
                }
                self.pending.set(self.pending.get() + 1);
                let pending = Pending(self.pending.clone());
                let controller = self.controller.clone();
                let _ = self.executor.spawner().spawn_local(async move {
                    let _pending = pending;
                    match command {
                        Command::Devices(reply) => {
                            let _ = reply.send(controller.devices().await);
                        }
                        Command::Inspect(id, reply) => {
                            let _ = reply.send(controller.inspect(&id).await);
                        }
                        Command::Capture {
                            device,
                            request,
                            incoming,
                            events,
                            cancel,
                            reply,
                        } => {
                            let _ = reply.send(
                                controller
                                    .capture(&device, &request, incoming, events, cancel)
                                    .await,
                            );
                        }
                        Command::Button {
                            device,
                            seconds,
                            cancel,
                            reply,
                        } => {
                            let _ = reply.send(controller.button(&device, seconds, cancel).await);
                        }
                        Command::Recover(reply) => {
                            let _ = reply.send(controller.recover());
                        }
                    }
                });
            }
            self.executor.run_until_stalled();
            // SAFETY: Host is main-thread-only, objects remain retained and the
            // documented default run-loop mode dispatches ImageCapture callbacks.
            unsafe {
                NSRunLoop::mainRunLoop().runMode_beforeDate(
                    NSDefaultRunLoopMode,
                    &NSDate::dateWithTimeIntervalSinceNow(0.02),
                );
            }
        });
    }
}
impl Drop for MacHost {
    fn drop(&mut self) {
        self.controller.stop();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
        while self.controller.active.get() && std::time::Instant::now() < deadline {
            self.tick();
        }
    }
}
fn unavailable() -> ScannerError {
    ScannerError::new(
        "driver",
        "Native scanner event loop is unavailable or full.",
    )
}
impl MacBackend {
    fn send(&self, command: Command) -> Result<()> {
        self.sender.try_send(command).map_err(|_| unavailable())
    }
}
#[async_trait]
impl ScannerBackend for MacBackend {
    async fn devices(&self) -> Result<Vec<Device>> {
        let (tx, rx) = oneshot::channel();
        self.send(Command::Devices(tx))?;
        rx.await.map_err(|_| unavailable())?
    }
    async fn inspect(&self, id: &str) -> Result<Inspection> {
        let (tx, rx) = oneshot::channel();
        self.send(Command::Inspect(id.into(), tx))?;
        rx.await.map_err(|_| unavailable())?
    }
    async fn capture(
        &self,
        id: &str,
        request: &ScanRequest,
        incoming: PathBuf,
        events: events::Sender<CaptureEvent>,
        cancel: CancellationToken,
    ) -> Result<CaptureOutcome> {
        let (tx, rx) = oneshot::channel();
        self.send(Command::Capture {
            device: id.into(),
            request: request.clone(),
            incoming,
            events,
            cancel,
            reply: tx,
        })?;
        rx.await.map_err(|_| unavailable())?
    }
    async fn wait_for_scan_button(&self, id: &str, cancel: CancellationToken) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.send(Command::Button {
            device: id.into(),
            seconds: None,
            cancel,
            reply: tx,
        })?;
        rx.await.map_err(|_| unavailable())?
    }
    async fn observe_button(
        &self,
        id: &str,
        seconds: u32,
        cancel: CancellationToken,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.send(Command::Button {
            device: id.into(),
            seconds: Some(seconds),
            cancel,
            reply: tx,
        })?;
        rx.await.map_err(|_| unavailable())?
    }
    async fn recover_after_disconnect(&self) -> bool {
        let (tx, rx) = oneshot::channel();
        if self.send(Command::Recover(tx)).is_err() {
            return false;
        }
        rx.await.unwrap_or(false)
    }
}

struct Pending(Rc<Cell<usize>>);
impl Drop for Pending {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}
impl Command {
    fn reject(self) {
        match self {
            Self::Devices(reply) => {
                let _ = reply.send(Err(unavailable()));
            }
            Self::Inspect(_, reply) => {
                let _ = reply.send(Err(unavailable()));
            }
            Self::Capture { reply, .. } => {
                let _ = reply.send(Err(unavailable()));
            }
            Self::Button { reply, .. } => {
                let _ = reply.send(Err(unavailable()));
            }
            Self::Recover(reply) => {
                let _ = reply.send(false);
            }
        }
    }
}
