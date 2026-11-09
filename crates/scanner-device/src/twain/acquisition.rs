use super::{
    configuration::Changes,
    frame::Frame,
    manager::{Manager, error},
    phase::Phase,
    transfer_abi::{BufferSizes, ImageInfo, ImageTransfer, Memory},
};
use crate::{CaptureEvent, CaptureOutcome, persist_settings};
use scanner_types::{
    error::{Result, ScannerError},
    scan::ScanRequest,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use tokio::{runtime::Handle, sync::mpsc};
use tokio_util::sync::CancellationToken;

pub(super) struct Capture<'a> {
    pub id: &'a str,
    pub request: &'a ScanRequest,
    pub incoming: &'a Path,
    pub events: &'a mpsc::Sender<CaptureEvent>,
    pub cancel: &'a CancellationToken,
    pub runtime: &'a Handle,
}
pub(super) fn capture(
    manager: &mut Manager,
    capture: Capture<'_>,
    mut pump: impl FnMut(&mut Manager) -> Result<bool>,
) -> CaptureOutcome {
    if let Err(error) = manager.open_source(capture.id) {
        return CaptureOutcome {
            errors: vec![error],
            cancelled: false,
            session_closed: manager.source.is_none(),
        };
    }
    let mut changes = Changes::default();
    let result = (|| {
        crate::native_output::validate_incoming(capture.incoming)?;
        let settings = changes.configure(manager, capture.request)?;
        capture
            .runtime
            .block_on(persist_settings(capture.events, settings))?;
        check_cancel(&capture, Instant::now())?;
        manager.enable()?;
        let started = Instant::now();
        while manager.phase == Phase::Enabled {
            check_progress(&capture, started, started)?;
            if pump(manager)? {
                return Err(ScannerError::new(
                    "cancelled",
                    "TWAIN source requested closure.",
                ));
            }
            if manager.phase == Phase::Enabled {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        for sequence in 1..=100 {
            check_cancel(&capture, started)?;
            let image = read_image(manager, &capture, started, &mut pump)?;
            let filename = crate::native_output::write_image(
                image,
                capture.request.format,
                capture.incoming,
                sequence,
            )?;
            capture.runtime.block_on(async {
                tokio::time::timeout(
                    Duration::from_secs(15),
                    capture
                        .events
                        .send(CaptureEvent::Image { filename, sequence }),
                )
                .await
                .map_err(|_| error("Captured-image delivery timed out."))?
                .map_err(|_| error("Coordinator disconnected."))
            })?;
            manager.end_transfer()?;
            if manager.phase == Phase::Enabled {
                if capture.request.duplex && sequence % 2 != 0 {
                    return Err(error("TWAIN duplex capture ended without a complete pair."));
                }
                return Ok(());
            }
        }
        Err(ScannerError::new(
            "resourceLimit",
            "TWAIN capture stopped at the 100-image limit.",
        ))
    })();
    finish_capture(manager, changes, result)
}
fn finish_capture(manager: &mut Manager, changes: Changes, result: Result<()>) -> CaptureOutcome {
    let mut errors: Vec<_> = result.err().into_iter().collect();
    if manager.quarantined {
        // An invalid native status leaves the transfer phase unproven. Retain
        // ownership instead of guessing a cleanup sequence.
    } else if let Err(error) = manager.stop_transfer() {
        errors.push(error);
    } else {
        errors.extend(changes.restore(manager));
        if let Err(error) = manager.close_source() {
            errors.push(error);
        }
        if errors.iter().any(|error| error.kind == "cleanup") {
            manager.quarantined = true;
        }
    }
    CaptureOutcome {
        cancelled: errors.iter().any(|error| error.kind == "cancelled"),
        session_closed: manager.source.is_none(),
        errors,
    }
}
fn check_cancel(capture: &Capture<'_>, started: Instant) -> Result<()> {
    if capture.cancel.is_cancelled() {
        return Err(ScannerError::new("cancelled", "Operation cancelled."));
    }
    if started.elapsed() > Duration::from_secs(1800) {
        return Err(ScannerError::new(
            "timeout",
            "TWAIN capture exceeded its batch deadline.",
        ));
    }
    Ok(())
}
fn read_image(
    manager: &mut Manager,
    capture: &Capture<'_>,
    started: Instant,
    pump: &mut impl FnMut(&mut Manager) -> Result<bool>,
) -> Result<image::DynamicImage> {
    let mut frame = read_frame_info(manager, capture.request.dpi)?;
    let mut sizes = BufferSizes::default();
    manager.source_call(1, 6, 1, (&mut sizes as *mut BufferSizes).cast())?;
    if sizes.minimum == 0 || sizes.maximum < sizes.minimum || sizes.minimum > 16 * 1024 * 1024 {
        return Err(error("Invalid TWAIN transfer buffer limits."));
    }
    let length = sizes
        .preferred
        .max(sizes.minimum)
        .min(sizes.maximum)
        .min(16 * 1024 * 1024);
    let mut buffer = vec![0u8; length as usize];
    let frame_started = Instant::now();
    loop {
        check_progress(capture, started, frame_started)?;
        if pump(manager)? {
            return Err(ScannerError::new(
                "cancelled",
                "TWAIN source requested closure.",
            ));
        }
        let mut transfer = ImageTransfer {
            memory: Memory {
                flags: 9,
                length,
                pointer: buffer.as_mut_ptr().cast(),
            },
            ..ImageTransfer::default()
        };
        let source = manager
            .source
            .as_mut()
            .ok_or_else(|| error("TWAIN source disappeared."))?;
        // SAFETY: bounded writable buffer remains live through this synchronous
        // call. Cancellation never closes or calls the source from another thread.
        let status = unsafe {
            (manager.entry)(
                &mut manager.app,
                source,
                2,
                0x103,
                1,
                (&mut transfer as *mut ImageTransfer).cast(),
            )
        };
        if let Err(error) = manager.phase.transfer_result(status) {
            manager.quarantined = true;
            return Err(error);
        }
        if !matches!(status, 0 | 6) {
            manager.check(status, true)?;
        }
        if transfer.memory.pointer != buffer.as_mut_ptr().cast() || transfer.memory.length != length
        {
            return Err(error("TWAIN replaced the application-owned buffer."));
        }
        let progressed = frame.append(&transfer, &buffer)?;
        if status == 6 {
            return frame.finish();
        }
        if !progressed {
            return Err(error("TWAIN transfer made no progress."));
        }
    }
}

fn read_frame_info(manager: &mut Manager, dpi: u32) -> Result<Frame> {
    let mut info = ImageInfo::default();
    manager.source_call(2, 0x101, 1, (&mut info as *mut ImageInfo).cast())?;
    if info.x_resolution.whole != dpi as i16
        || info.y_resolution.whole != dpi as i16
        || info.x_resolution.fraction != 0
        || info.y_resolution.fraction != 0
    {
        return Err(error(
            "TWAIN image DPI differs from the requested resolution.",
        ));
    }
    Frame::new(info)
}

fn check_progress(capture: &Capture<'_>, started: Instant, frame_started: Instant) -> Result<()> {
    check_cancel(capture, started)?;
    if frame_started.elapsed() > Duration::from_secs(120) {
        return Err(ScannerError::new(
            "timeout",
            "TWAIN frame exceeded its progress deadline.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use libloading::Library;
    #[test]
    fn native_capture_checks_settings_pixels_and_every_cleanup_state() {
        let _guard = super::super::manager::FIXTURE_LOCK.lock().unwrap();
        let Some(path) = std::env::var_os("SCANNER_TWAIN_TEST_LIBRARY") else {
            return;
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        // SAFETY: explicit test-only fixture, no Win32 window or hardware.
        unsafe {
            let fixture = Library::new(&path).unwrap();
            let configure = *fixture
                .get::<unsafe extern "C" fn(i32)>(b"fixture_configure\0")
                .unwrap();
            let counter = *fixture
                .get::<unsafe extern "C" fn(i32) -> i32>(b"fixture_counter\0")
                .unwrap();
            for scenario in [0, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
                configure(if matches!(scenario, 8 | 9) {
                    0
                } else {
                    scenario
                });
                let mut manager = Manager::load(Path::new(&path), std::ptr::null_mut()).unwrap();
                manager.initialize().unwrap();
                let identity = manager.identities().unwrap().remove(0);
                let device = manager.device(identity);
                let directory = tempfile::tempdir().unwrap();
                let (events, receiver) = mpsc::channel(8);
                let consumer = consume_events(&runtime, receiver, scenario, counter);
                let cancel = CancellationToken::new();
                if scenario == 9 {
                    cancel.cancel();
                }
                let request = ScanRequest {
                    duplex: true,
                    ..ScanRequest::default()
                };
                let outcome = capture(
                    &mut manager,
                    Capture {
                        id: &device.id,
                        request: &request,
                        incoming: directory.path(),
                        events: &events,
                        cancel: &cancel,
                        runtime: runtime.handle(),
                    },
                    |manager| {
                        manager
                            .process_event(std::ptr::null_mut())
                            .map(|(_, close)| close)
                    },
                );
                drop(events);
                let images = runtime.block_on(consumer).unwrap();
                assert_scenario(scenario, &outcome, &images, counter);
                check_images(directory.path(), &images);
                if manager.quarantined {
                    std::mem::forget(manager);
                } else {
                    manager.shutdown().unwrap();
                }
            }
        }
    }
    fn consume_events(
        runtime: &tokio::runtime::Runtime,
        mut receiver: mpsc::Receiver<CaptureEvent>,
        scenario: i32,
        counter: unsafe extern "C" fn(i32) -> i32,
    ) -> tokio::task::JoinHandle<Vec<(String, u32)>> {
        runtime.spawn(async move {
            let mut images = Vec::new();
            while let Some(event) = receiver.recv().await {
                match event {
                    CaptureEvent::Settings { acknowledged, .. } => {
                        assert_eq!(unsafe { counter(6) }, 0, "Feed preceded durable settings");
                        let result = if scenario == 8 {
                            Err(ScannerError::new("storage", "Simulated disk failure"))
                        } else {
                            Ok(())
                        };
                        acknowledged.send(result).unwrap();
                    }
                    CaptureEvent::Image { filename, sequence } => images.push((filename, sequence)),
                }
            }
            images
        })
    }
    fn check_images(directory: &Path, images: &[(String, u32)]) {
        for (filename, _) in images {
            let image = image::open(directory.join(filename)).unwrap().into_rgb8();
            assert_eq!(image.dimensions(), (2, 2));
            assert_eq!(image.as_raw(), &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        }
    }
    unsafe fn assert_scenario(
        scenario: i32,
        outcome: &CaptureOutcome,
        images: &[(String, u32)],
        counter: unsafe extern "C" fn(i32) -> i32,
    ) {
        // SAFETY: caller loaded the known fixture and completed native work.
        unsafe {
            assert_eq!(counter(5), 0, "Invalid transition in scenario {scenario}");
            assert_eq!(counter(2), 0, "Leaked capability memory");
            assert_eq!(counter(3), 0, "Leaked lock");

            if scenario == 0 {
                assert!(outcome.errors.is_empty());
                assert_eq!(images.len(), 2);
                assert_eq!(images[0].1, 1);
                assert_eq!(images[1].1, 2);
            } else {
                assert!(
                    !outcome.errors.is_empty(),
                    "Expected failure for scenario {scenario}"
                );
            }
            if matches!(scenario, 6 | 10) {
                assert!(!outcome.session_closed);
                assert_eq!(counter(9), 0);
            } else {
                assert!(outcome.session_closed);
                assert_eq!(counter(10), 300);
            }
            if matches!(scenario, 2 | 8 | 9) {
                assert_eq!(counter(6), 0);
                assert!(images.is_empty());
            }
            if scenario == 4 {
                assert_eq!(counter(7), 0);
                assert_eq!(counter(8), 1);
            }
            if matches!(scenario, 3 | 5) {
                assert_eq!(counter(7), 1);
                assert_eq!(counter(8), 1);
            }
            if matches!(scenario, 3 | 9) {
                assert!(outcome.cancelled);
            }
            if scenario == 7 {
                assert_eq!(images.len(), 1);
            }
        }
    }
}
