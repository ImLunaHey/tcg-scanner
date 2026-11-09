use super::{
    ffi::{self, Parameters},
    frames::Frame,
    session::{Capture, Session},
};
use crate::CaptureEvent;
use image::ImageFormat;
use scanner_types::{
    error::{Result, ScannerError},
    scan::OriginalFormat,
};
use std::{fs::OpenOptions, io::Cursor, time::Duration};

pub(super) fn parameters(session: &Session<'_>) -> Result<Frame> {
    let mut parameters = Parameters::default();
    // SAFETY: worker owns this open handle and a writable parameter structure.
    unsafe {
        ffi::check(
            (session.api.parameters)(session.handle, &mut parameters),
            "read frame parameters",
        )?;
    }
    Frame::new(parameters)
}
pub(super) fn capture_pages(session: &Session<'_>, capture: &Capture<'_>) -> Result<()> {
    let metadata = std::fs::symlink_metadata(capture.incoming).map_err(ffi::error)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ffi::error("Invalid incoming directory."));
    }
    for sequence in 1..=100 {
        if let Some(error) = capture.cancellation.failure() {
            return Err(error);
        }
        // SAFETY: settings are persisted and no other normal SANE call runs.
        let status = unsafe { (session.api.start)(session.handle) };
        if status == 7 && sequence > 1 {
            if capture.request.duplex && (sequence - 1) % 2 != 0 {
                return Err(ffi::error(
                    "Duplex capture ended without a complete front/back pair.",
                ));
            }
            return Ok(());
        }
        ffi::check(status, "start scan")?;
        let mut frame = parameters(session)?;
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            if let Some(error) = capture.cancellation.failure() {
                return Err(error);
            }
            let mut length = 0;
            // SAFETY: writable buffer and length match the SANE read signature.
            let status = unsafe {
                (session.api.read)(
                    session.handle,
                    buffer.as_mut_ptr(),
                    buffer.len() as i32,
                    &mut length,
                )
            };
            if length < 0 || length as usize > buffer.len() {
                return Err(ffi::error("Invalid SANE read length."));
            }
            if status == 5 {
                if length != 0 {
                    return Err(ffi::error(
                        "SANE returned data alongside end-of-frame status.",
                    ));
                }
                break;
            }
            ffi::check(status, "read scan")?;
            if length == 0 {
                return Err(ffi::error("Blocking SANE read made no progress."));
            }
            frame.push(&buffer[..length as usize])?;
        }
        let filename = write_frame(frame, capture, sequence)?;
        capture.cancellation.received_frame();
        capture.runtime.block_on(async {
            tokio::time::timeout(
                Duration::from_secs(15),
                capture
                    .events
                    .send(CaptureEvent::Image { filename, sequence }),
            )
            .await
            .map_err(|_| {
                ScannerError::new("timeout", "Coordinator did not accept captured image.")
            })?
            .map_err(|_| {
                ScannerError::new(
                    "storage",
                    "Coordinator disconnected while preserving captured image.",
                )
            })
        })?;
    }
    Err(ScannerError::new(
        "resourceLimit",
        "Capture stopped at the 100-image batch limit.",
    ))
}
fn write_frame(frame: Frame, capture: &Capture<'_>, sequence: u32) -> Result<String> {
    let image = frame.finish()?;
    let mut encoded = Cursor::new(Vec::new());
    let format = match capture.request.format {
        OriginalFormat::Png => ImageFormat::Png,
        OriginalFormat::Tiff => ImageFormat::Tiff,
    };
    image.write_to(&mut encoded, format).map_err(ffi::error)?;
    let filename = format!(
        "capture-{sequence:04}.{}",
        capture.request.format.extension()
    );
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(capture.incoming.join(&filename))
        .map_err(ffi::error)?;
    use std::io::Write;
    file.write_all(encoded.get_ref()).map_err(ffi::error)?;
    file.sync_all().map_err(ffi::error)?;
    // Keep even an oversized complete original in private staging for recovery.
    if encoded.get_ref().len() > 32 * 1024 * 1024 {
        return Err(ScannerError::new(
            "resourceLimit",
            "Captured image exceeds the original-file import limit; retained in incoming storage.",
        ));
    }
    Ok(filename)
}
