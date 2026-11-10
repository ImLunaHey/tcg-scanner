//! Immutable, manifest-verified browser bundle, loaded before serving requests.
mod files;
use axum::{
    body::{Body, Bytes},
    http::{header, request::Parts},
    response::Response,
};
use scanner_types::error::{Result, ScannerError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebManifest {
    pub schema_version: u32,
    pub files: BTreeMap<String, String>,
}
struct Asset {
    bytes: Bytes,
    media_type: &'static str,
    etag: String,
}
pub struct WebAssets {
    assets: BTreeMap<String, Asset>,
}
impl WebAssets {
    pub fn load(directory: &Path) -> Result<Self> {
        let root = cap_std::fs::Dir::open_ambient_dir(directory, cap_std::ambient_authority())
            .map_err(invalid)?;
        let manifest: WebManifest =
            serde_json::from_slice(&files::read(&root, "manifest.json", 65536)?)
                .map_err(invalid)?;
        if manifest.schema_version != 1 || !(5..=64).contains(&manifest.files.len()) {
            return Err(invalid("Invalid web manifest version or file count."));
        }
        for required in [
            "index.html",
            "app.js",
            "app.css",
            "scanner_ui.js",
            "scanner_ui_bg.wasm",
        ] {
            if !manifest.files.contains_key(required) {
                return Err(invalid("Incomplete browser bundle."));
            }
        }
        let mut assets = BTreeMap::new();
        let mut total = 0;
        for (name, digest) in manifest.files {
            let (media_type, limit) = file_type(&name)?;
            let bytes = files::read(&root, &name, limit)?;
            total += bytes.len();
            if total > 68 * 1024 * 1024 || digest != format!("{:x}", Sha256::digest(&bytes)) {
                return Err(invalid("Browser bundle digest or size is invalid."));
            }
            if name.ends_with(".wasm") && !bytes.starts_with(b"\0asm\x01\0\0\0") {
                return Err(invalid("Invalid WebAssembly module."));
            }
            assets.insert(
                name,
                Asset {
                    bytes: bytes.into(),
                    media_type,
                    etag: format!("\"{digest}\""),
                },
            );
        }
        Ok(Self { assets })
    }
    pub(crate) fn contains(&self, path: &crate::path::Path) -> bool {
        self.assets.contains_key(&name(path))
    }
    pub(crate) fn response(
        &self,
        head: &Parts,
        path: &crate::path::Path,
    ) -> crate::error::Result<Response> {
        path.only(&[])?;
        let asset = self
            .assets
            .get(&name(path))
            .ok_or_else(crate::error::Error::internal)?;
        let matched = crate::gateway::header_value(head, header::IF_NONE_MATCH)?
            .is_some_and(|v| v == asset.etag);
        let body = if matched || head.method == "HEAD" {
            Body::empty()
        } else {
            Body::from(asset.bytes.clone())
        };
        let mut response = Response::new(body);
        *response.status_mut() = if matched {
            axum::http::StatusCode::NOT_MODIFIED
        } else {
            axum::http::StatusCode::OK
        };
        let headers = response.headers_mut();
        headers.insert(header::CONTENT_TYPE, asset.media_type.parse().unwrap());
        headers.insert(header::ETAG, asset.etag.parse().unwrap());
        headers.insert(header::CACHE_CONTROL, "no-cache".parse().unwrap());
        if !matched {
            headers.insert(header::CONTENT_LENGTH, asset.bytes.len().into());
        }
        headers.insert(header::CONTENT_SECURITY_POLICY, CSP.parse().unwrap());
        Ok(response)
    }
}
const CSP: &str = "default-src 'none'; manifest-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; media-src 'self' blob:; img-src 'self' blob: data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; worker-src 'self' blob:; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
fn name(path: &crate::path::Path) -> String {
    if path.parts.is_empty() || path.parts == ["camera"] {
        "index.html".into()
    } else {
        path.parts.join("/")
    }
}
pub fn file_type(name: &str) -> Result<(&'static str, usize)> {
    if name.len() > 512
        || name.split('/').count() > 12
        || name.split('/').any(|part| {
            part.is_empty()
                || matches!(part, "." | "..")
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
    {
        return Err(invalid("Invalid browser asset path."));
    }
    match name.rsplit('.').next() {
        Some("html") if name == "index.html" => Ok(("text/html; charset=utf-8", 65536)),
        Some("js") => Ok(("text/javascript; charset=utf-8", 2 * 1024 * 1024)),
        Some("ttf") => Ok(("font/ttf", 2 * 1024 * 1024)),
        Some("txt") if name == "Inter-LICENSE.txt" => Ok(("text/plain; charset=utf-8", 65536)),
        Some("css") => Ok(("text/css; charset=utf-8", 65536)),
        Some("webmanifest") if name == "camera.webmanifest" => {
            Ok(("application/manifest+json", 8192))
        }
        Some("png") if matches!(name, "camera-192.png" | "camera-512.png") => {
            Ok(("image/png", 1024 * 1024))
        }
        Some("wasm") if matches!(name, "scanner_ui_bg.wasm" | "scanner_worker_bg.wasm") => {
            Ok(("application/wasm", 64 * 1024 * 1024))
        }
        _ => Err(invalid("Unsupported browser asset type.")),
    }
}
fn invalid(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("webAssets", error.to_string())
}
