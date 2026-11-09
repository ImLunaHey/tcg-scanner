use super::{
    browser::Controller,
    connection::{Connection, cancelled},
};
use objc2_foundation::NSNumber;
use objc2_image_capture_core::{ICScannerFeatureEnumeration, ICScannerFunctionalUnitType};
use scanner_types::error::{Result, ScannerError};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
impl Controller {
    pub async fn button(
        &self,
        id: &str,
        seconds: Option<u32>,
        cancel: CancellationToken,
    ) -> Result<()> {
        if seconds.is_some_and(|seconds| !(30..=300).contains(&seconds)) {
            return Err(ScannerError::new(
                "invalidParams",
                "Button observation must last 30–300 seconds.",
            ));
        }
        let _owner = self.acquire()?;
        let connection = self.connection(id)?;
        let result = wait(&connection, seconds, &cancel).await;
        connection.signals.borrow_mut().button_armed = false;
        let cleanup = connection.close().await;
        let closed = connection.signals.borrow().closed;
        if !closed {
            *self.quarantined.borrow_mut() = Some(connection);
        }
        if let Some(error) = cleanup
            .iter()
            .find(|e| matches!(e.kind.as_str(), "cleanup" | "closeSession"))
            .or_else(|| cleanup.first())
        {
            return Err(error.clone());
        }
        if !closed {
            return Err(ScannerError::new(
                "cleanup",
                "Button-monitor session did not close.",
            ));
        }
        result
    }
}
async fn wait(
    connection: &Connection,
    seconds: Option<u32>,
    cancel: &CancellationToken,
) -> Result<()> {
    connection.connect(cancel).await?;
    connection
        .select(ICScannerFunctionalUnitType::DocumentFeeder, cancel)
        .await?;
    if seconds.is_none() {
        // Read the driver's pre-enabled monitor only. Never alter transport,
        // multifeed, paper-thickness or other safety features to enable it.
        let enabled = unsafe {
            connection
                .scanner
                .selectedFunctionalUnit()
                .vendorFeatures()
                .and_then(|features| {
                    features.iter().find(|f| {
                        f.internalName()
                            .is_some_and(|name| name.to_string() == "scan_button_monitor")
                    })
                })
                .and_then(|f| f.downcast::<ICScannerFeatureEnumeration>().ok())
                .and_then(|feature| feature.currentValue().downcast::<NSNumber>().ok())
                .is_some_and(|number| number.intValue() == 1)
        };
        if !enabled {
            return Err(ScannerError::new(
                "unsupported",
                "Enable Scan button monitor in the official driver before using automatic batches.",
            ));
        }
        connection.signals.borrow_mut().button_armed = true;
    }
    let started = Instant::now();
    loop {
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        {
            let state = connection.signals.borrow();
            if let Some(error) = &state.error {
                return Err(error.clone());
            }
            if state.button_accepted
                || seconds
                    .is_some_and(|seconds| started.elapsed() >= Duration::from_secs(seconds.into()))
            {
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
