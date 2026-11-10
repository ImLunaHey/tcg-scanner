use super::{
    browser::{Controller, describe},
    configuration::Configuration,
    connection::{Connection, cancelled},
    snapshot::snapshot,
};
use crate::{CaptureEvent, CaptureOutcome, persist_settings};
use objc2_image_capture_core::ICScannerFunctionalUnitType;
use scanner_types::{
    device::Inspection,
    error::{Result, ScannerError},
    scan::ScanRequest,
};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

impl Controller {
    pub async fn inspect(&self, id: &str) -> Result<Inspection> {
        let _owner = self.acquire()?;
        let connection = self.connection(id)?;
        let mut report = Inspection::new(describe(&connection.scanner));
        let cancel = CancellationToken::new();
        let result = async {
            connection.connect(&cancel).await?;
            let units = unsafe { connection.scanner.availableFunctionalUnitTypes() };
            if units.len() > 32 {
                return Err(ScannerError::new(
                    "resourceLimit",
                    "Too many functional units exposed by driver.",
                ));
            }
            report.available_functional_unit_types =
                units.iter().map(|n| n.unsignedIntValue()).collect();
            for &unit in &report.available_functional_unit_types {
                connection
                    .select(ICScannerFunctionalUnitType(unit as usize), &cancel)
                    .await?;
                let selected = unsafe { connection.scanner.selectedFunctionalUnit() };
                report.units.push(snapshot(&selected));
            }
            Ok::<_, ScannerError>(())
        }
        .await;
        if let Err(error) = result {
            report.errors.push(error);
        }
        for error in connection.close().await {
            if !report.errors.contains(&error) {
                report.errors.push(error);
            }
        }
        {
            let state = connection.signals.borrow();
            report.session_opened = state.opened;
            report.session_closed = state.closed;
        }
        report.unknown_capabilities = [
            "firmware",
            "maintenanceCounters",
            "paperThickness",
            "continuousScanning",
            "supportedPixelModes",
        ]
        .map(str::to_owned)
        .to_vec();
        report.limitations.push("Writable properties are driver-exposed, not hardware-tested; enum codes are ImageCaptureCore values.".into());
        if !report.session_closed {
            *self.quarantined.borrow_mut() = Some(connection);
        }
        Ok(report)
    }
    pub async fn capture(
        &self,
        id: &str,
        request: &ScanRequest,
        incoming: PathBuf,
        events: mpsc::Sender<CaptureEvent>,
        cancel: CancellationToken,
    ) -> Result<CaptureOutcome> {
        request.validate()?;
        let _owner = self.acquire()?;
        if cancel.is_cancelled() {
            return Ok(CaptureOutcome {
                errors: vec![],
                cancelled: true,
                session_closed: true,
            });
        }
        let connection = self.connection(id)?;
        let mut configuration = Configuration::default();
        let result = scan(
            &connection,
            request,
            &incoming,
            &events,
            &cancel,
            &mut configuration,
        )
        .await;
        let mut outcome = CaptureOutcome {
            errors: vec![],
            cancelled: cancel.is_cancelled(),
            session_closed: false,
        };
        if let Err(error) = result {
            if error.kind == "cancelled" {
                outcome.cancelled = true;
            } else {
                outcome.errors.push(error);
            }
        }
        cancel_transfer(&connection, &mut outcome).await;
        if let Err(error) = forward_files(&connection, &incoming, &events).await {
            outcome.errors.push(error);
        }
        let stopped = {
            let s = connection.signals.borrow();
            !s.removed && (!s.scan_requested || s.scan_completed)
        };
        if stopped {
            configuration.restore();
        }
        for error in connection.close().await {
            if !outcome.errors.contains(&error) {
                outcome.errors.push(error);
            }
        }
        outcome.session_closed = connection.signals.borrow().closed;
        if !outcome.session_closed
            || outcome
                .errors
                .iter()
                .any(|e| matches!(e.kind.as_str(), "cleanup" | "closeSession"))
        {
            *self.quarantined.borrow_mut() = Some(connection);
        }
        Ok(outcome)
    }
}
async fn scan(
    connection: &Connection,
    request: &ScanRequest,
    incoming: &Path,
    events: &mpsc::Sender<CaptureEvent>,
    cancel: &CancellationToken,
    configuration: &mut Configuration,
) -> Result<()> {
    connection.connect(cancel).await?;
    let has_adf = unsafe {
        connection
            .scanner
            .availableFunctionalUnitTypes()
            .iter()
            .any(|n| n.unsignedIntValue() == 3)
    };
    if !has_adf {
        return Err(ScannerError::new(
            "unsupportedSetting",
            "No ADF functional unit available.",
        ));
    }
    connection
        .select(ICScannerFunctionalUnitType::DocumentFeeder, cancel)
        .await?;
    let settings = configuration.apply(&connection.scanner, request, incoming)?;
    persist_settings(events, settings).await?;
    if cancel.is_cancelled() {
        return Err(cancelled());
    }
    {
        let mut state = connection.signals.borrow_mut();
        state.scan_requested = true;
        state.last_image = Instant::now();
    }
    // SAFETY: explicit settings were validated and durably acknowledged; only
    // this owner requests a feed, and every exit path cancels/awaits closure.
    unsafe {
        connection.scanner.requestScan();
    }
    let started = Instant::now();
    loop {
        forward_files(connection, incoming, events).await?;
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        {
            let state = connection.signals.borrow();
            if let Some(error) = &state.error {
                return Err(error.clone());
            }
            if state.scan_completed {
                return Ok(());
            }
            if state.last_image.elapsed() >= Duration::from_secs(120)
                || started.elapsed() >= Duration::from_secs(1800)
            {
                return Err(ScannerError::new(
                    "timeout",
                    "Scan exceeded 120 seconds without a new image or the 30-minute batch limit.",
                ));
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
async fn cancel_transfer(connection: &Connection, outcome: &mut CaptureOutcome) {
    let unfinished = {
        let s = connection.signals.borrow();
        s.scan_requested && !s.scan_completed && !s.removed
    };
    if !unfinished {
        return;
    }
    unsafe {
        connection.scanner.cancelScan();
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    while !connection.signals.borrow().scan_completed
        && !connection.signals.borrow().removed
        && Instant::now() < deadline
    {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let s = connection.signals.borrow();
    if !s.scan_completed && !s.removed {
        outcome.errors.push(ScannerError::new(
            "cleanup",
            "Scan cancellation was not acknowledged; reconnect before another scan.",
        ));
    }
}
async fn forward_files(
    connection: &Connection,
    incoming: &Path,
    events: &mpsc::Sender<CaptureEvent>,
) -> Result<()> {
    loop {
        let next = connection.signals.borrow_mut().files.pop_front();
        let Some((sequence, path)) = next else {
            return Ok(());
        };
        if path.parent() != Some(incoming) {
            return Err(ScannerError::new(
                "storage",
                "Driver returned a file outside its staging directory.",
            ));
        }
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| ScannerError::new("storage", "Invalid driver filename."))?;
        send(
            events,
            CaptureEvent::Image {
                filename: filename.into(),
                sequence,
            },
        )
        .await?;
    }
}
async fn send(events: &mpsc::Sender<CaptureEvent>, event: CaptureEvent) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(15), events.send(event))
        .await
        .map_err(|_| {
            ScannerError::new(
                "storage",
                "Capture queue did not drain; originals remain in staging.",
            )
        })?
        .map_err(|_| {
            ScannerError::new(
                "storage",
                "Capture receiver disconnected; originals remain in staging.",
            )
        })
}
