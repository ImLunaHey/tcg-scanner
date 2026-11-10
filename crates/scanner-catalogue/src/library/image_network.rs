use scanner_types::error::{Result, ScannerError};
#[async_trait::async_trait]
pub trait ImageTransport: Send + Sync {
    async fn get(&self, url: &str, limit: usize) -> Result<Vec<u8>>;
}
pub(crate) struct Https(reqwest::Client);
impl Https {
    pub fn new() -> Result<Self> {
        Ok(Self(
            reqwest::Client::builder()
                .https_only(true)
                .redirect(reqwest::redirect::Policy::none())
                .referer(false)
                .connect_timeout(std::time::Duration::from_secs(12))
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .map_err(super::unavailable)?,
        ))
    }
}
#[async_trait::async_trait]
impl ImageTransport for Https {
    async fn get(&self, url: &str, limit: usize) -> Result<Vec<u8>> {
        if !super::references::allowed(url) && !crate::reference::allowed(url) {
            return Err(ScannerError::new(
                "reference",
                "Unsupported catalogue image URL.",
            ));
        }
        let mut response = self.0.get(url).send().await.map_err(super::unavailable)?;
        let mime = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .and_then(|v| v.split(';').next())
            .unwrap_or("");
        if response.status() != reqwest::StatusCode::OK
            || response.url().as_str() != url
            || !matches!(mime, "image/png" | "image/jpeg")
            || response.content_length().is_some_and(|n| n > limit as u64)
        {
            return Err(ScannerError::new(
                "referenceUnavailable",
                "Catalogue preview unavailable.",
            ));
        }
        let mut bytes = vec![];
        while let Some(chunk) = response.chunk().await.map_err(super::unavailable)? {
            if bytes.len() + chunk.len() > limit {
                return Err(ScannerError::new(
                    "resourceLimit",
                    "Catalogue image is too large.",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}
