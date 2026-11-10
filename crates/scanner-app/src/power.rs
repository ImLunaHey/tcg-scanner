use scanner_core::Engine;
use scanner_platform::power::{Event, Monitor};
use scanner_types::error::{Result, ScannerError};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub(crate) async fn run(
    engine: Arc<Engine>,
    monitor: Option<Monitor>,
    stop: CancellationToken,
) -> Result<()> {
    let Some(mut monitor) = monitor else {
        stop.cancelled().await;
        return Ok(());
    };
    let result = loop {
        let event = tokio::select! {
            _ = stop.cancelled() => break Ok(()),
            event = monitor.next() => event,
        };
        match event {
            Some(Event::Suspend(ack)) => {
                let result = engine.prepare_for_sleep().await;
                drop(ack);
                if let Err(error) = result {
                    break Err(error);
                }
            }
            Some(Event::Resume) => {
                if let Err(error) = engine.resume_after_wake().await {
                    break Err(error);
                }
            }
            Some(Event::Failed(error)) => break Err(error),
            None => {
                break Err(ScannerError::new(
                    "powerMonitor",
                    "Native power notifications stopped.",
                ));
            }
        }
    };
    monitor.shutdown().await;
    result
}
