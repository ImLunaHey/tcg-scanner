use crate::{Transport, service_origin, transport};
use scanner_types::{
    camera::{CameraIdentification, CameraRequest, MAX_IMAGE_BYTES, MAX_REQUEST_BYTES},
    error::{Result, ScannerError},
};
/// Narrow anonymous camera client; it has no library or session authority.
#[derive(Clone)]
pub struct CameraClient {
    http: Transport,
    endpoint: url::Url,
}
impl CameraClient {
    pub fn new(origin: &str) -> Result<Self> {
        let mut endpoint = service_origin(origin)?;
        endpoint.set_path("/api/v1/camera/identify");
        Ok(Self {
            http: Transport::new()?,
            endpoint,
        })
    }
    pub async fn identify(&self, request: CameraRequest) -> Result<CameraIdentification> {
        if request.base64.is_empty() || request.base64.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4 {
            return Err(ScannerError::new(
                "resourceLimit",
                "Camera image exceeds 512 KiB.",
            ));
        }
        let body = serde_json::to_vec(&request).map_err(transport)?;
        if body.len() > MAX_REQUEST_BYTES {
            return Err(ScannerError::new(
                "resourceLimit",
                "Camera request exceeds 768 KiB.",
            ));
        }
        let bytes = self.http.post(&self.endpoint, "", body).await?;
        if bytes.len() > 1024 * 1024 {
            return Err(ScannerError::new(
                "resourceLimit",
                "Camera result exceeds 1 MiB.",
            ));
        }
        let result: CameraIdentification =
            serde_json::from_slice(&bytes).map_err(|_| transport("Invalid camera result."))?;
        if result.schema_version != 1 || result.candidates.len() > 10 {
            return Err(transport("Unsupported camera result."));
        }
        Ok(result)
    }
}
pub(crate) fn status_error(status: u16) -> ScannerError {
    match status {
        409 => ScannerError::new("busy", "The scanner service is busy. Hold steady to retry."),
        429 => ScannerError::new(
            "rateLimit",
            "Camera lookup limit reached. Pause before retrying.",
        ),
        _ => ScannerError::new(
            "http",
            format!("Camera lookup failed with HTTP status {status}."),
        ),
    }
}
