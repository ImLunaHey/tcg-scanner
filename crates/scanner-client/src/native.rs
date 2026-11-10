use crate::transport;
use scanner_types::error::{Result, ScannerError};
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct Transport(reqwest::Client);
impl Transport {
    pub fn new() -> Result<Self> {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(120))
            .build()
            .map(Self)
            .map_err(transport)
    }
    pub async fn post(&self, endpoint: &url::Url, token: &str, body: Vec<u8>) -> Result<Vec<u8>> {
        let mut authorization = reqwest::header::HeaderValue::from_str(token).map_err(transport)?;
        authorization.set_sensitive(true);
        let mut response = self
            .0
            .post(endpoint.clone())
            .header(reqwest::header::AUTHORIZATION, authorization)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(transport)?;
        if !response.status().is_success() {
            return Err(transport(format!(
                "Scanner HTTP request failed with status {}.",
                response.status().as_u16()
            )));
        }
        if response
            .content_length()
            .is_some_and(|n| n > 16 * 1024 * 1024)
        {
            return Err(limit());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport)? {
            if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
                return Err(limit());
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}
fn limit() -> ScannerError {
    ScannerError::new("resourceLimit", "Service response exceeds 16 MiB.")
}
