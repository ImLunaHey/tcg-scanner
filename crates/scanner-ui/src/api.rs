use base64::{Engine, engine::general_purpose::STANDARD};
use scanner_client::Client;
use scanner_types::{
    error::{Result, ScannerError},
    image_access::ImageThumbnail,
    service::{JobPage, ServiceStatus},
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub struct Connected {
    pub client: Client,
    pub status: ServiceStatus,
    pub page: JobPage,
}
pub async fn connect(client: Client) -> Result<Connected> {
    let status = call(&client, "service.status", json!({})).await?;
    let page = call(&client, "jobs.list", json!({"limit":100})).await?;
    Ok(Connected {
        client,
        status,
        page,
    })
}
pub async fn call<T: DeserializeOwned>(client: &Client, method: &str, params: Value) -> Result<T> {
    serde_json::from_value(client.call(method, params).await?)
        .map_err(|_| ScannerError::new("transport", "Invalid typed service response."))
}
pub async fn thumbnail(client: &Client, source: &crate::library::ImageSource) -> Result<Vec<u8>> {
    thumbnail_params(client, source.params()).await
}
pub async fn thumbnail_params(client: &Client, params: Value) -> Result<Vec<u8>> {
    let thumbnail: ImageThumbnail = call(client, "images.thumbnail", params).await?;
    decode_thumbnail(thumbnail)
}
pub async fn reference(client: &Client, key: &str) -> Result<Vec<u8>> {
    let thumbnail: ImageThumbnail = call(client, "inventory.reference", json!({"key":key})).await?;
    crate::reference_preview::decode(thumbnail)
}

fn decode_thumbnail(thumbnail: ImageThumbnail) -> Result<Vec<u8>> {
    let invalid = || ScannerError::new("image", "Invalid service thumbnail.");
    if thumbnail.schema_version != 1
        || thumbnail.media_type != "image/png"
        || thumbnail.width == 0
        || thumbnail.height == 0
        || thumbnail.width > 1024
        || thumbnail.height > 1024
        || thumbnail.base64.len() > 3 * 1024 * 1024
    {
        return Err(invalid());
    }
    let bytes = STANDARD.decode(thumbnail.base64).map_err(|_| invalid())?;
    if format!("{:x}", Sha256::digest(&bytes)) != thumbnail.sha256
        || bytes.len() > 2 * 1024 * 1024
        || !bytes.starts_with(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR")
        || bytes.get(16..20) != Some(thumbnail.width.to_be_bytes().as_slice())
        || bytes.get(20..24) != Some(thumbnail.height.to_be_bytes().as_slice())
    {
        return Err(invalid());
    }
    Ok(bytes)
}
#[cfg(not(target_arch = "wasm32"))]
pub fn initial_origin() -> String {
    std::env::var("SCANNER_UI_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:8171".into())
}
#[cfg(target_arch = "wasm32")]
pub fn initial_origin() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    fn thumbnail(width: u32) -> ImageThumbnail {
        // Only the envelope/header checks run here; image decoding belongs to Rune.
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&40u32.to_be_bytes());
        ImageThumbnail {
            schema_version: 1,
            base64: STANDARD.encode(&bytes),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            media_type: "image/png".into(),
            width,
            height: 40,
        }
    }
    #[test]
    fn rejects_corrupt_content_and_oversized_or_misdeclared_dimensions() {
        assert!(decode_thumbnail(thumbnail(30)).is_ok());
        let mut bad = thumbnail(30);
        bad.sha256 = "0".repeat(64);
        assert!(decode_thumbnail(bad).is_err());
        let mut bomb = thumbnail(100_000);
        bomb.width = 30;
        assert!(decode_thumbnail(bomb).is_err());
        assert!(decode_thumbnail(thumbnail(2048)).is_err());
        let mut wrong = thumbnail(30);
        wrong.media_type = "image/jpeg".into();
        assert!(decode_thumbnail(wrong).is_err());
    }
}
