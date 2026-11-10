//! Native suspend/resume notifications. No shell, helper or UI dependency.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
use scanner_types::error::{Result, ScannerError};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

pub enum Event {
    Suspend(Acknowledgement),
    Resume,
    Failed(ScannerError),
}
/// Releases the OS's bounded sleep delay, including when an event is discarded.
/// Operating systems may suspend before cleanup finishes; this is not a veto.
#[derive(Default)]
pub struct Acknowledgement(Option<Box<dyn FnOnce() + Send>>);
impl Acknowledgement {
    fn new(action: impl FnOnce() + Send + 'static) -> Self {
        Self(Some(Box::new(action)))
    }
}
impl Drop for Acknowledgement {
    fn drop(&mut self) {
        if let Some(action) = self.0.take() {
            action();
        }
    }
}
pub struct Monitor {
    receiver: mpsc::UnboundedReceiver<Event>,
    stop: CancellationToken,
    worker: Option<tokio::task::JoinHandle<()>>,
}
impl Monitor {
    pub async fn start() -> Result<Self> {
        // Only trusted local OS callbacks produce events; retain their order even
        // when cleanup spans a sleep/wake cycle.
        let (sender, receiver) = mpsc::unbounded_channel();
        let (ready, started) = oneshot::channel();
        let stop = CancellationToken::new();
        let token = stop.clone();
        let worker = tokio::spawn(async move {
            let result = native(sender.clone(), token, ready).await;
            if let Err(error) = result {
                let _ = sender.send(Event::Failed(error));
            }
        });
        let mut monitor = Self {
            receiver,
            stop,
            worker: Some(worker),
        };
        let result = match tokio::time::timeout(std::time::Duration::from_secs(10), started).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => match monitor.receiver.recv().await {
                Some(Event::Failed(error)) => Err(error),
                _ => Err(error("Power monitor could not register.")),
            },
            Err(_) => Err(error("Power monitor startup timed out.")),
        };
        if let Err(error) = result {
            monitor.stop.cancel();
            if let Some(worker) = monitor.worker.take() {
                let _ = worker.await;
            }
            return Err(error);
        }
        Ok(monitor)
    }
    pub async fn next(&mut self) -> Option<Event> {
        self.receiver.recv().await
    }
    pub async fn shutdown(mut self) {
        self.stop.cancel();
        self.receiver.close();
        while self.receiver.try_recv().is_ok() {}
        if let Some(worker) = self.worker.take() {
            let _ = worker.await;
        }
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}
#[cfg(any(target_os = "macos", windows))]
async fn native(
    sender: mpsc::UnboundedSender<Event>,
    stop: CancellationToken,
    ready: oneshot::Sender<Result<()>>,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        return macos::run(sender, stop, ready);
        #[cfg(windows)]
        return windows::run(sender, stop, ready);
    })
    .await
    .map_err(error)?
}
#[cfg(target_os = "linux")]
async fn native(
    sender: mpsc::UnboundedSender<Event>,
    stop: CancellationToken,
    ready: oneshot::Sender<Result<()>>,
) -> Result<()> {
    tokio::select! {
        _ = stop.cancelled() => Ok(()),
        result = linux::run(sender, stop.clone(), ready) => result,
    }
}
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
async fn native(
    _: mpsc::UnboundedSender<Event>,
    _: CancellationToken,
    ready: oneshot::Sender<Result<()>>,
) -> Result<()> {
    let _ = ready.send(Err(error("Native power notifications are unavailable.")));
    Ok(())
}
fn error(value: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("powerMonitor", value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dropped_events_release_sleep_acknowledgements_once() {
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = count.clone();
        let (sender, receiver) = mpsc::unbounded_channel();
        sender
            .send(Event::Suspend(Acknowledgement::new(move || {
                observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            })))
            .unwrap_or_else(|_| panic!("receiver closed"));
        drop(receiver);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
