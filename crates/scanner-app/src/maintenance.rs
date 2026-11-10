use scanner_core::Engine;
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

pub(crate) async fn run(engine: Arc<Engine>, stop: CancellationToken) {
    let mut ticks = tokio::time::interval(Duration::from_secs(2));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_error = None;
    loop {
        tokio::select! { biased;
            _ = stop.cancelled() => break,
            _ = ticks.tick() => {}
        }
        let status = engine.status().await;
        if status.stopping || status.paused {
            continue;
        }
        engine.recover_connection().await;
        match engine.devices().await {
            Ok(_) => {
                last_error = None;
                engine.maintain_scan_button().await;
            }
            Err(error) => {
                let message = error.to_string();
                if last_error.as_ref() != Some(&message) {
                    eprintln!("Device monitor: {message}");
                }
                last_error = Some(message);
            }
        }
    }
}
