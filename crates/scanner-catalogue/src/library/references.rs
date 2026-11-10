use super::ImageTransport;
use crate::cache::Cache;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use scanner_types::{
    error::{Result, ScannerError},
    image_access::ImageThumbnail,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Cursor,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tokio::sync::Semaphore;
pub(super) struct References {
    urls: Mutex<BTreeMap<String, String>>,
    documents: Option<Arc<crate::persistent::Documents>>,
    images: Arc<dyn ImageTransport>,
    cache: Cache<ImageThumbnail>,
    workers: Arc<Semaphore>,
}
impl References {
    pub fn new(images: Arc<dyn ImageTransport>) -> Self {
        Self {
            urls: Default::default(),
            documents: None,
            images,
            cache: Cache::new(128, Duration::from_secs(7 * 86400)),
            workers: Arc::new(Semaphore::new(2)),
        }
    }
    pub fn with_documents(mut self, documents: Arc<crate::persistent::Documents>) -> Self {
        self.documents = Some(documents);
        self
    }
    pub fn register(&self, url: &str) -> Option<String> {
        if !allowed(url) {
            return None;
        }
        let key = format!("{:x}", Sha256::digest(url.as_bytes()));
        let mut urls = self.urls.lock().ok()?;
        if urls.len() >= 100_000 && !urls.contains_key(&key) {
            return None;
        }
        urls.insert(key.clone(), url.into());
        Some(key)
    }
    pub async fn thumbnail(&self, key: &str) -> Result<ImageThumbnail> {
        if key.len() != 64
            || !key
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(ScannerError::new(
                "invalidParams",
                "Invalid catalogue thumbnail key.",
            ));
        }
        let result = self
            .cache
            .get(key, || async {
                let permit = self.workers.clone().try_acquire_owned().map_err(|_| {
                    ScannerError::new(
                        "resourceLimit",
                        "Catalogue previews are busy; retry shortly.",
                    )
                })?;
                let url = self
                    .urls
                    .lock()
                    .map_err(super::unavailable)?
                    .get(key)
                    .cloned()
                    .ok_or_else(|| {
                        ScannerError::new(
                            "imageNotFound",
                            "Reload the set checklist to request this catalogue image.",
                        )
                    })?;
                let cache_key = format!("reference/{key}");
                if let Some(documents) = &self.documents
                    && let Ok(Some(bytes)) = documents
                        .get(&cache_key, Duration::from_secs(7 * 86400), 768 * 1024)
                        .await
                    && let Some(thumbnail) = restored(&bytes)
                {
                    return Ok(thumbnail);
                }
                let bytes = self.images.get(&url, 8 * 1024 * 1024).await?;
                let thumbnail = tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    encode(&bytes)
                })
                .await
                .map_err(super::unavailable)??;
                if let Some(documents) = &self.documents
                    && let Ok(bytes) = serde_json::to_vec(&thumbnail)
                {
                    let _ = documents.put(&cache_key, bytes).await;
                }
                Ok(thumbnail)
            })
            .await?;
        Ok(result.as_ref().clone())
    }
}
fn restored(bytes: &[u8]) -> Option<ImageThumbnail> {
    let value: ImageThumbnail = serde_json::from_slice(bytes).ok()?;
    let image = STANDARD.decode(&value.base64).ok()?;
    (value.schema_version == 1
        && value.media_type == "image/jpeg"
        && (1..=512).contains(&value.width)
        && (1..=512).contains(&value.height)
        && image.len() <= 512 * 1024
        && format!("{:x}", Sha256::digest(&image)) == value.sha256)
        .then_some(value)
}
pub(super) fn allowed(url: &str) -> bool {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    url.len()<=512 && !url.split('/').any(|s|matches!(s,"."|"..")) && PATTERN.get_or_init(||regex::Regex::new(r"\Ahttps://(?:assets\.tcgdex\.net/(?:en|ko)/[A-Za-z0-9._+\-]+/[A-Za-z0-9._+\-]+/[A-Za-z0-9._+\-]+/low\.png|images\.ygoprodeck\.com/images/cards/[0-9]{1,10}\.jpg|cdn\.swu-db\.com/images/cards/[A-Z0-9]{3,8}/[A-Za-z0-9_\-]+\.png)\z").unwrap()).is_match(url)
}
fn encode(bytes: &[u8]) -> Result<ImageThumbnail> {
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(ScannerError::new(
            "resourceLimit",
            "Catalogue image exceeds 8 MiB.",
        ));
    }
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(super::unavailable)?;
    let format = reader.format();
    if !matches!(
        format,
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
    ) {
        return Err(ScannerError::new(
            "reference",
            "Catalogue image must be PNG or JPEG.",
        ));
    }
    let (w, h) = reader.into_dimensions().map_err(super::unavailable)?;
    if w == 0 || h == 0 || w > 8000 || h > 8000 || u64::from(w) * u64::from(h) > 16_000_000 {
        return Err(ScannerError::new(
            "resourceLimit",
            "Catalogue image dimensions exceed bounds.",
        ));
    }
    if format == Some(image::ImageFormat::Png)
        && image::codecs::png::PngDecoder::new(Cursor::new(bytes))
            .map_err(super::unavailable)?
            .is_apng()
            .map_err(super::unavailable)?
    {
        return Err(ScannerError::new(
            "reference",
            "Animated catalogue images are unsupported.",
        ));
    }
    let image = scanner_images::thumbnail::thumbnail(bytes, 512, true)?;
    if image.bytes.len() > 512 * 1024 {
        return Err(ScannerError::new(
            "resourceLimit",
            "Catalogue thumbnail exceeds 512 KiB.",
        ));
    }
    Ok(ImageThumbnail {
        schema_version: 1,
        base64: STANDARD.encode(image.bytes),
        sha256: image.sha256,
        media_type: image.media_type.into(),
        width: image.width,
        height: image.height,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registered_urls_cannot_escape_the_catalogue_image_allowlist() {
        for url in [
            "https://assets.tcgdex.net/en/sv/sv09/177/low.png",
            "https://images.ygoprodeck.com/images/cards/123.jpg",
            "https://cdn.swu-db.com/images/cards/SOR/001.png",
        ] {
            assert!(allowed(url));
        }
        for url in [
            "http://assets.tcgdex.net/en/sv/sv09/177/low.png",
            "https://assets.tcgdex.net:443/en/sv/sv09/177/low.png",
            "https://assets.tcgdex.net/en/../sv09/177/low.png",
            "https://assets.tcgdex.net/en/sv/sv09/177/low.png?x=1",
            "https://assets.tcgdex.net/en/sv/sv09/177/low.png\n",
            "https://localhost/images/cards/1.jpg",
            "https://user@images.ygoprodeck.com/images/cards/1.jpg",
        ] {
            assert!(!allowed(url));
        }
    }
    struct Preview(std::sync::atomic::AtomicUsize);
    #[async_trait::async_trait]
    impl ImageTransport for Preview {
        async fn get(&self, _: &str, _: usize) -> Result<Vec<u8>> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            scanner_images::encode_png(image::RgbaImage::from_pixel(
                10,
                15,
                image::Rgba([20, 40, 60, 255]),
            ))
            .map(|p| p.bytes)
        }
    }
    #[tokio::test]
    async fn reopened_cache_serves_previews_without_another_image_download() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .canonicalize()
            .unwrap()
            .join("private/public.sqlite");
        let transport = Arc::new(Preview(std::sync::atomic::AtomicUsize::new(0)));
        let url = "https://images.ygoprodeck.com/images/cards/123.jpg";
        let checksum = {
            let references = References::new(transport.clone())
                .with_documents(crate::persistent::Documents::open(&path).unwrap());
            let key = references.register(url).unwrap();
            references.thumbnail(&key).await.unwrap().sha256
        };
        let references = References::new(transport.clone())
            .with_documents(crate::persistent::Documents::open(&path).unwrap());
        let key = references.register(url).unwrap();
        assert_eq!(references.thumbnail(&key).await.unwrap().sha256, checksum);
        assert_eq!(transport.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
