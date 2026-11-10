use async_trait::async_trait;
use scanner_types::error::{Result, ScannerError};
use std::time::Duration;
use tokio::sync::Semaphore;

/// Injectable for offline catalogues and deterministic tests. Paths are generated
/// by the client; implementations must enforce the supplied decoded-byte limit.
#[async_trait]
pub trait JsonTransport: Send + Sync {
    async fn get(&self, path: &str, limit: usize) -> Result<Vec<u8>>;
}
pub(crate) struct Https {
    client: reqwest::Client,
    origin: &'static str,
    requests: Semaphore,
}
impl Https {
    pub fn new() -> Result<Self> {
        Self::with_origin("https://api.tcgdex.net/v2/", 20)
    }
    pub fn yugioh() -> Result<Self> {
        Self::with_origin("https://db.ygoprodeck.com/api/v7/", 60)
    }
    pub fn star_wars() -> Result<Self> {
        Self::with_origin("https://api.swu-db.com/", 25)
    }
    fn with_origin(origin: &'static str, timeout: u64) -> Result<Self> {
        let client = reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .connect_timeout(Duration::from_secs(12))
            .timeout(Duration::from_secs(timeout))
            .pool_max_idle_per_host(2)
            .user_agent("Scanner/1 (local card identification)")
            .build()
            .map_err(unavailable)?;
        Ok(Self {
            client,
            origin,
            requests: Semaphore::new(2),
        })
    }
}
#[async_trait]
impl JsonTransport for Https {
    async fn get(&self, path: &str, limit: usize) -> Result<Vec<u8>> {
        let _permit = self.requests.acquire().await.map_err(unavailable)?;
        let url = format!("{}{path}", self.origin);
        let mut response = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(unavailable)?;
        if response.status() != reqwest::StatusCode::OK
            || response.url().as_str() != url
            || response.content_length().is_some_and(|n| n > limit as u64)
        {
            return Err(ScannerError::new(
                "catalogueUnavailable",
                "Catalogue returned an unsuccessful or oversized response. Retry later.",
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(unavailable)? {
            append(&mut bytes, &chunk, limit)?;
        }
        Ok(bytes)
    }
}
fn unavailable(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("catalogueUnavailable", error.to_string())
}
fn append(bytes: &mut Vec<u8>, chunk: &[u8], limit: usize) -> Result<()> {
    if bytes.len().saturating_add(chunk.len()) > limit {
        return Err(ScannerError::new(
            "catalogue",
            "Catalogue response exceeds the size limit.",
        ));
    }
    bytes.extend_from_slice(chunk);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunked_responses_cannot_bypass_the_body_limit() {
        let mut body = vec![1; 4];
        assert!(append(&mut body, &[2; 4], 8).is_ok());
        assert!(append(&mut body, &[3], 8).is_err());
        assert_eq!(body.len(), 8);
    }
}
