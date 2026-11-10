use crate::Engine;
use base64::Engine as _;
use scanner_types::{
    camera::{CameraIdentification, CameraRequest, MAX_IMAGE_BYTES},
    error::{Result, ScannerError},
};
use std::{sync::Arc, time::Duration};
impl Engine {
    /// Own the bounded lookup after admission, even if its HTTP caller disconnects.
    /// Camera work never touches the scan ledger, storage or scanner backend.
    pub async fn identify_camera(
        self: &Arc<Self>,
        request: CameraRequest,
    ) -> Result<CameraIdentification> {
        let bytes = decode(&request.base64)?;
        let recognizer = self.recognizer.clone().ok_or_else(|| {
            ScannerError::new("configuration", "Native recognition is not configured.")
        })?;
        let permit = self
            .recognition_workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| busy())?;
        {
            let mut state = self.state.lock().await;
            state.accepting()?;
            if state.camera || state.inspecting || state.active.is_some() || !state.queue.is_empty()
            {
                return Err(busy());
            }
            state.camera = true;
        }
        let (reply, response) = tokio::sync::oneshot::channel();
        let engine = self.clone();
        tokio::spawn(async move {
            // A panic in an adapter must still release the reservation and wake
            // queued scans/shutdown. Native worker permits outlive cancelled callers.
            let result = match engine.release_button().await {
                Ok(()) => tokio::spawn(async move {
                    recognizer.recognize_camera(bytes, request.language).await
                })
                .await
                .map_err(crate::image_access::worker)
                .and_then(|v| v),
                Err(error) => Err(error),
            };
            drop(permit);
            engine.state.lock().await.camera = false;
            engine.wake.notify_one();
            engine.idle.notify_waiters();
            let _ = reply.send(result);
        });
        tokio::time::timeout(Duration::from_secs(60), response)
            .await
            .map_err(|_| {
                ScannerError::new(
                    "timeout",
                    "Camera identification timed out. Try a clearer view.",
                )
            })?
            .map_err(crate::image_access::worker)?
    }
}
fn decode(value: &str) -> Result<Vec<u8>> {
    let invalid = || {
        ScannerError::new(
            "invalidParams",
            "Camera image must be base64 JPEG up to 512 KiB.",
        )
    };
    if value.is_empty() || value.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4 {
        return Err(invalid());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| invalid())?;
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(invalid());
    }
    Ok(bytes)
}
fn busy() -> ScannerError {
    ScannerError::new("busy", "Camera identification is busy. Try again shortly.")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_bad_encoding_and_bounds_both_encoded_and_decoded_bytes() {
        assert!(decode("").is_err());
        assert!(decode("not-base64!").is_err());
        assert!(decode(&"A".repeat(MAX_IMAGE_BYTES.div_ceil(3) * 4 + 1)).is_err());
        let encoded = base64::engine::general_purpose::STANDARD.encode(vec![1; MAX_IMAGE_BYTES]);
        assert_eq!(decode(&encoded).unwrap().len(), MAX_IMAGE_BYTES);
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(vec![1; MAX_IMAGE_BYTES + 1]);
        assert!(decode(&encoded).is_err());
    }
}
