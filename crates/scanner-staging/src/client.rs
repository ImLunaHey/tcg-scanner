use crate::{
    Staging,
    contracts::{CardInput, CardReply, Import, ImportReply},
};
use reqwest::{
    Method,
    header::{AUTHORIZATION, HeaderValue},
};
use scanner_types::error::{Result, ScannerError};
use serde::{Serialize, de::DeserializeOwned};
use std::time::Duration;
use url::Url;

const RESPONSE_LIMIT: usize = 2 * 1024 * 1024;
const PHOTO_LIMIT: usize = 10 * 1024 * 1024;

/// HTTPS-only, redirect-free Store client with bounded responses and uploads.
/// The caller owns secure credential loading; credentials are never in errors.
pub struct Client {
    origin: String,
    instance: String,
    authorization: HeaderValue,
    http: reqwest::Client,
    requests: tokio::sync::Semaphore,
}
impl Client {
    pub fn new(origin: &str, instance: String, token: &str) -> Result<Self> {
        let origin = validated_origin(origin)?;
        if instance.is_empty()
            || instance.len() > 128
            || !(32..=1024).contains(&token.len())
            || !token.bytes().all(|b| (33..=126).contains(&b))
        {
            return Err(error(
                "storeConfiguration",
                "Invalid Store instance or credential.",
            ));
        }
        let mut authorization = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| error("storeConfiguration", "Invalid Store credential."))?;
        authorization.set_sensitive(true);
        let http = reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(90))
            .build()
            .map_err(|_| unavailable())?;
        Ok(Self {
            origin,
            instance,
            authorization,
            http,
            requests: tokio::sync::Semaphore::new(1),
        })
    }
    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<T> {
        let _permit = self.requests.acquire().await.map_err(|_| unavailable())?;
        let url = format!("{}{path}", self.origin);
        let mut response = self
            .http
            .request(method, &url)
            .header(AUTHORIZATION, self.authorization.clone())
            .header("Content-Type", content_type)
            .header("Accept", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| unavailable())?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(error(
                if status == 409 {
                    "storeConflict"
                } else {
                    "storeHTTP"
                },
                format!("Store returned HTTP {status}. Review the staged card and retry."),
            ));
        }
        if response.url().as_str() != url
            || response
                .content_length()
                .is_some_and(|n| n > RESPONSE_LIMIT as u64)
        {
            return Err(error(
                "storeResponse",
                "Unexpected or oversized Store response.",
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
            if bytes.len() + chunk.len() > RESPONSE_LIMIT {
                return Err(error(
                    "storeResponse",
                    "Store response exceeds the allowed size.",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| {
            error(
                "storeResponse",
                "Store returned an unexpected staging response.",
            )
        })
    }
}
#[async_trait::async_trait]
impl Staging for Client {
    fn origin(&self) -> &str {
        &self.origin
    }
    fn scanner_instance_id(&self) -> &str {
        &self.instance
    }
    async fn import(&self, value: &Import) -> Result<ImportReply> {
        self.request(
            Method::POST,
            "/api/v1/admin/scan-batches",
            json(value)?,
            "application/json",
        )
        .await
    }
    async fn update(&self, card: &CardReply, input: &CardInput) -> Result<CardReply> {
        let body = serde_json::json!({
            "revision":card.revision,"status":"pending","notes":"","confirmed":null,
            "review":input.review,"game":input.game,"language":input.language,
            "gameSource":input.game_source,"languageSource":input.language_source,
        });
        self.request(
            Method::PUT,
            &format!("/api/v1/admin/scan-cards/{}", card.id),
            json(&body)?,
            "application/json",
        )
        .await
    }
    async fn upload(&self, card: &CardReply, front: Vec<u8>, back: Vec<u8>) -> Result<CardReply> {
        let (body, content_type) = multipart(card.revision, &front, &back)?;
        self.request(
            Method::POST,
            &format!("/api/v1/admin/scan-cards/{}/photos", card.id),
            body,
            &content_type,
        )
        .await
    }
}
pub(crate) fn validated_origin(value: &str) -> Result<String> {
    let url =
        Url::parse(value).map_err(|_| error("storeConfiguration", "Invalid Store origin."))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err(error(
            "storeConfiguration",
            "Store requires an HTTPS origin without a path or credentials.",
        ));
    }
    Ok(url.origin().ascii_serialization())
}
/// Resolve only same-origin HTTPS review links returned by the staging service.
pub fn review_url(origin: &str, path: &str) -> Result<String> {
    let origin = validated_origin(origin)?;
    let base = Url::parse(&origin).map_err(|_| error("storeResponse", "Invalid Store origin."))?;
    let url = base
        .join(path)
        .map_err(|_| error("storeResponse", "Unsafe Store review link."))?;
    if url.origin() != base.origin() || !url.username().is_empty() || url.password().is_some() {
        return Err(error("storeResponse", "Unsafe Store review link."));
    }
    Ok(url.into())
}
fn json(value: &impl Serialize) -> Result<Vec<u8>> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| error("storeRequest", "Invalid staging payload."))?;
    if bytes.len() > 1024 * 1024 {
        return Err(error("resourceLimit", "Staging metadata exceeds 1 MiB."));
    }
    Ok(bytes)
}
fn multipart(revision: u64, front: &[u8], back: &[u8]) -> Result<(Vec<u8>, String)> {
    let signature = b"\x89PNG\r\n\x1a\n";
    if [front, back]
        .iter()
        .any(|v| v.len() > PHOTO_LIMIT || !v.starts_with(signature))
    {
        return Err(error(
            "resourceLimit",
            "Store requires two prepared PNGs of at most 10 MiB each.",
        ));
    }
    let boundary = format!("scanner-{}", uuid::Uuid::new_v4());
    let mut bytes = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"revision\"\r\n\r\n{revision}\r\n"
    )
    .into_bytes();
    for (side, data) in [("front", front), ("back", back)] {
        bytes.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{side}\"; filename=\"{side}.png\"\r\nContent-Type: image/png\r\n\r\n").as_bytes());
        bytes.extend_from_slice(data);
        bytes.extend_from_slice(b"\r\n");
    }
    bytes.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Ok((bytes, format!("multipart/form-data; boundary={boundary}")))
}
fn error(kind: &str, message: impl Into<String>) -> ScannerError {
    ScannerError::new(kind, message)
}
fn unavailable() -> ScannerError {
    error(
        "storeUnavailable",
        "Store could not be reached. Retry the transfer; scans are preserved.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn respond(wire: String) -> (Result<serde_json::Value>, String) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n{}") {
                let mut bytes = [0; 1024];
                let n = socket.read(&mut bytes).await.unwrap();
                assert!(n > 0 && request.len() < 4096);
                request.extend_from_slice(&bytes[..n]);
            }
            let _ = socket.write_all(wire.as_bytes()).await;
            String::from_utf8(request).unwrap()
        });
        let mut client =
            Client::new("https://store.example", "scanner".into(), &"x".repeat(32)).unwrap();
        client.origin = format!("http://{address}");
        client.http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let result = client
            .request(Method::POST, "/fixture", b"{}".to_vec(), "application/json")
            .await;
        (result, server.await.unwrap())
    }
    #[tokio::test]
    async fn http_bounds_redirects_and_failures_do_not_expose_upstream_data() {
        let (ok, request) =
            respond("HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\n{\"ok\":true}".into()).await;
        assert_eq!(ok.unwrap(), serde_json::json!({"ok":true}));
        assert!(request.starts_with("POST /fixture HTTP/1.1\r\n"));
        assert!(request.to_lowercase().contains("authorization: bearer "));
        for (wire, kind) in [
            (
                "HTTP/1.1 409 Conflict\r\nContent-Length: 6\r\n\r\nSECRET".to_owned(),
                "storeConflict",
            ),
            (
                "HTTP/1.1 302 Found\r\nLocation: https://evil.example\r\nContent-Length: 0\r\n\r\n"
                    .to_owned(),
                "storeHTTP",
            ),
            (
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                    RESPONSE_LIMIT + 1
                ),
                "storeResponse",
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nSECRET".to_owned(),
                "storeResponse",
            ),
            (
                format!(
                    "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
                    RESPONSE_LIMIT + 1,
                    "x".repeat(RESPONSE_LIMIT + 1)
                ),
                "storeResponse",
            ),
        ] {
            let error = respond(wire).await.0.unwrap_err();
            assert_eq!(error.kind, kind);
            assert!(!error.message.contains("SECRET"));
            assert!(!error.message.contains(&"x".repeat(32)));
        }
    }
    #[test]
    fn credentials_origins_and_review_links_are_confined() {
        let origin = "https://store.example";
        assert!(Client::new(origin, "scanner".into(), &"x".repeat(32)).is_ok());
        for value in [
            "http://store.example",
            "https://user@store.example",
            "https://store.example/path",
            "https://store.example?token=x",
            "https://store.example#x",
        ] {
            assert!(Client::new(value, "scanner".into(), &"x".repeat(32)).is_err());
        }
        assert!(Client::new(origin, "scanner".into(), &format!("{}\n", "x".repeat(32))).is_err());
        assert_eq!(
            review_url(origin, "/review/1").unwrap(),
            "https://store.example/review/1"
        );
        for path in [
            "//evil.example/a",
            "https://store.example:444/a",
            "http://store.example/a",
            "https://user@store.example/a",
        ] {
            assert!(review_url(origin, path).is_err());
        }
    }
    #[test]
    fn paired_upload_carries_revision_and_rejects_non_png_or_oversize() {
        let png = b"\x89PNG\r\n\x1a\n";
        let (body, kind) = multipart(19, png, png).unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(kind.starts_with("multipart/form-data; boundary=scanner-"));
        assert!(text.contains("name=\"revision\"\r\n\r\n19\r\n"));
        assert_eq!(text.matches("Content-Type: image/png").count(), 2);
        assert!(multipart(19, png, b"jpeg").is_err());
        let mut large = vec![0; PHOTO_LIMIT + 1];
        large[..8].copy_from_slice(png);
        assert!(multipart(19, &large, png).is_err());
    }
}
