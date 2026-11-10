//! Bounded public artwork for visual comparison; never a physical-finish label.
use crate::{JsonTransport, TcgDex, cache::Cache, library::ImageTransport, persistent::Documents};
use scanner_types::{
    cards::CardLanguage,
    error::{Result, ScannerError},
};
use sha2::{Digest, Sha256};
use std::{
    io::Cursor,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tokio::sync::Semaphore;
const LIMIT: usize = 8 * 1024 * 1024;
const TTL: Duration = Duration::from_secs(7 * 86400);
#[derive(Clone)]
pub struct ReferenceImage {
    pub url: String,
    pub sha256: String,
    pub bytes: Vec<u8>,
}
pub struct Artwork {
    catalogue: TcgDex,
    images: Arc<dyn ImageTransport>,
    documents: Option<Arc<Documents>>,
    cache: Cache<Option<ReferenceImage>>,
    workers: Arc<Semaphore>,
}
impl Artwork {
    pub fn new() -> Result<Self> {
        Ok(Self::with_transports(
            Arc::new(crate::transport::Https::new()?),
            Arc::new(crate::library::image_network::Https::new()?),
        ))
    }
    pub fn with_transports(json: Arc<dyn JsonTransport>, images: Arc<dyn ImageTransport>) -> Self {
        Self {
            catalogue: TcgDex::with_transport(CardLanguage::English, json),
            images,
            documents: None,
            cache: Cache::new(8, TTL),
            workers: Arc::new(Semaphore::new(2)),
        }
    }
    pub fn with_documents(mut self, documents: Arc<Documents>) -> Self {
        self.catalogue = self.catalogue.with_documents(documents.clone());
        self.documents = Some(documents);
        self
    }
    pub async fn for_card(&self, id: &str) -> Result<Arc<Option<ReferenceImage>>> {
        self.cache
            .get(id, || async {
                let permit = self.workers.clone().try_acquire_owned().map_err(error)?;
                let detail = self.catalogue.detail(id).await?;
                let Some(base) = &detail.image else {
                    return Ok(None);
                };
                let url = format!("{base}/high.png");
                if !allowed(&url) {
                    return Err(error("Unsupported public reference URL."));
                }
                let key = format!("reference-original/{:x}", Sha256::digest(url.as_bytes()));
                let mut permit = permit;
                if let Some(documents) = &self.documents
                    && let Ok(Some(bytes)) = documents.get(&key, TTL, LIMIT).await
                {
                    let (parsed, returned) = checked(url.clone(), bytes, permit).await?;
                    permit = returned;
                    if let Ok(value) = parsed {
                        return Ok(Some(value));
                    }
                }
                let bytes = self.images.get(&url, LIMIT).await?;
                let (result, _permit) = checked(url, bytes, permit).await?;
                let result = result?;
                if let Some(documents) = &self.documents {
                    let _ = documents.put(&key, result.bytes.clone()).await;
                }
                Ok(Some(result))
            })
            .await
    }
}
pub(crate) fn allowed(url: &str) -> bool {
    static PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
        r"\Ahttps://assets\.tcgdex\.net/en/[A-Za-z0-9._+\-]+/[A-Za-z0-9._+\-]+/[A-Za-z0-9._+\-]+/high\.png\z").unwrap()
    });
    url.len() <= 512 && !url.split('/').any(|p| matches!(p, "." | "..")) && PATTERN.is_match(url)
}
async fn checked(
    url: String,
    bytes: Vec<u8>,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<(Result<ReferenceImage>, tokio::sync::OwnedSemaphorePermit)> {
    tokio::task::spawn_blocking(move || {
        let result = validate(&bytes).map(|()| ReferenceImage {
            url,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            bytes,
        });
        (result, permit)
    })
    .await
    .map_err(error)
}
fn validate(bytes: &[u8]) -> Result<()> {
    use image::ImageDecoder;
    if bytes.is_empty() || bytes.len() > LIMIT {
        return Err(error("Reference exceeds 8 MiB."));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8000);
    limits.max_image_height = Some(8000);
    limits.max_alloc = Some(128 * 1024 * 1024);
    let decoder =
        image::codecs::png::PngDecoder::with_limits(Cursor::new(bytes), limits).map_err(error)?;
    let (w, h) = decoder.dimensions();
    if w == 0
        || h == 0
        || w > 8000
        || h > 8000
        || u64::from(w) * u64::from(h) > 16_000_000
        || decoder.is_apng().map_err(error)?
    {
        return Err(error(
            "Reference must be one PNG frame of at most 16 megapixels.",
        ));
    }
    Ok(())
}
fn error(value: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("referenceUnavailable", value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_fixed_english_catalogue_artwork_urls_are_accepted() {
        let url = "https://assets.tcgdex.net/en/sv/sv01/104/high.png";
        assert!(allowed(url));
        for bad in [
            url.replace("assets.tcgdex.net", "localhost"),
            url.replace("/sv/", "/../"),
            format!("{url}?redirect=localhost"),
            url.replace("https:", "http:"),
            url.replace("/en/", "/ko/"),
            url.replace("104/", "%2e%2e/"),
            url.replace("/high.png", "/low.png"),
        ] {
            assert!(!allowed(&bad));
        }
        assert!(validate(b"not png").is_err());
    }
}
