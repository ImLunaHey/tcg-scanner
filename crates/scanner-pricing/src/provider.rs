use crate::{quotes::PricingDetail, rate};
use scanner_catalogue::{TcgDex, persistent::Documents};
use scanner_types::{
    cards::CardLanguage,
    error::{Result, ScannerError},
    pricing::{CardMarketPrices, EuroSterlingRate},
};
use std::{sync::Arc, time::Duration};
#[async_trait::async_trait]
pub trait PricingProvider: Send + Sync {
    async fn prices(&self, card_id: &str) -> Result<CardMarketPrices>;
    async fn sterling_rate(&self) -> Result<EuroSterlingRate>;
}
pub struct PublicPricing {
    catalogue: TcgDex,
    client: reqwest::Client,
    rates: tokio::sync::Semaphore,
}
const ECB: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";
impl PublicPricing {
    pub fn new() -> Result<Self> {
        Ok(Self {
            catalogue: TcgDex::new(CardLanguage::English)?,
            rates: tokio::sync::Semaphore::new(1),
            client: reqwest::Client::builder()
                .https_only(true)
                .redirect(reqwest::redirect::Policy::none())
                .referer(false)
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(15))
                .build()
                .map_err(error)?,
        })
    }
    pub fn with_documents(mut self, documents: Arc<Documents>) -> Self {
        self.catalogue = self.catalogue.with_documents(documents);
        self
    }
}
#[async_trait::async_trait]
impl PricingProvider for PublicPricing {
    async fn prices(&self, id: &str) -> Result<CardMarketPrices> {
        let detail = self.catalogue.detail(id).await?;
        let pricing: PricingDetail =
            serde_json::from_value(serde_json::to_value(detail.as_ref()).map_err(error)?)
                .map_err(error)?;
        if pricing.id != id {
            return Err(error("Catalogue pricing identity does not match."));
        }
        Ok(pricing.quotes(chrono::Utc::now()))
    }
    async fn sterling_rate(&self) -> Result<EuroSterlingRate> {
        let _permit = self.rates.acquire().await.map_err(error)?;
        let mut response = self.client.get(ECB).send().await.map_err(error)?;
        if response.status() != reqwest::StatusCode::OK
            || response.url().as_str() != ECB
            || response.content_length().is_some_and(|n| n > 64 * 1024)
        {
            return Err(error("ECB exchange rate unavailable."));
        }
        let mut bytes = vec![];
        while let Some(chunk) = response.chunk().await.map_err(error)? {
            if bytes.len() + chunk.len() > 64 * 1024 {
                return Err(error("Oversized ECB response."));
            }
            bytes.extend_from_slice(&chunk);
        }
        rate::decode(&bytes, chrono::Utc::now())
    }
}
fn error(value: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("pricingUnavailable", value.to_string())
}
