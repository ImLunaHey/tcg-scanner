//! Portable HTTP client for CLI, MCP and Rune consumers. Credentials are never
//! sent through URLs or redirects. This crate neither opens files nor runs helpers.
use scanner_types::error::{Result, ScannerError};
use serde_json::{Value, json};
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
use browser::Transport;
#[cfg(not(target_arch = "wasm32"))]
use native::Transport;
use uuid::Uuid;

#[derive(Clone)]
pub struct Client {
    http: Transport,
    endpoint: url::Url,
    authorization: String,
}
impl Client {
    pub fn new(origin: &str, token: &str) -> Result<Self> {
        let mut endpoint =
            url::Url::parse(origin).map_err(|_| invalid("Invalid service origin."))?;
        if !matches!(endpoint.scheme(), "https" | "http")
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || endpoint.path() != "/"
        {
            return Err(invalid(
                "Service origin must contain only an HTTP(S) scheme and authority.",
            ));
        }
        if endpoint.scheme() == "http" && !private_host(&endpoint) {
            return Err(invalid(
                "Plain HTTP is restricted to loopback or Tailscale IPv4.",
            ));
        }
        if token.len() != 64
            || !token
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid("Invalid service credential."));
        }
        endpoint.set_path("/rpc");
        let authorization = format!("Bearer {token}");
        let http = Transport::new()?;
        Ok(Self {
            http,
            endpoint,
            authorization,
        })
    }
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        if !params.is_object() {
            return Err(ScannerError::new(
                "invalidParams",
                "RPC parameters must be an object.",
            ));
        }
        let id = Uuid::new_v4().to_string();
        let method = if method.starts_with("v1.") {
            method.to_owned()
        } else {
            format!("v1.{method}")
        };
        let bytes =
            serde_json::to_vec(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
                .map_err(transport)?;
        if bytes.len() > 65536 {
            return Err(ScannerError::new(
                "resourceLimit",
                "RPC request exceeds 64 KiB.",
            ));
        }
        let bytes = self
            .http
            .post(&self.endpoint, &self.authorization, bytes)
            .await?;
        let response: Value =
            serde_json::from_slice(&bytes).map_err(|_| transport("Invalid service JSON."))?;
        if response["jsonrpc"] != "2.0"
            || response["id"] != id
            || response.get("result").is_some() == response.get("error").is_some()
        {
            return Err(transport("Invalid service response envelope."));
        }
        if let Some(error) = response.get("error") {
            return Err(
                serde_json::from_value(error["data"].clone()).unwrap_or_else(|_| {
                    ScannerError::new(
                        "api",
                        error["message"]
                            .as_str()
                            .unwrap_or("Service operation failed."),
                    )
                }),
            );
        }
        Ok(response["result"].clone())
    }
}
fn private_host(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain("localhost")) => true,
        Some(url::Host::Ipv4(ip)) => {
            ip.is_loopback() || ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1])
        }
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    }
}
fn invalid(message: &str) -> ScannerError {
    ScannerError::new("configuration", message)
}
fn transport(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("transport", error.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_require_private_http_or_explicit_https_without_url_auth() {
        let token = "a".repeat(64);
        for origin in [
            "http://127.0.0.1:8171",
            "http://[::1]:8171",
            "http://100.70.2.3:8170",
            "https://scanner.example",
        ] {
            assert!(Client::new(origin, &token).is_ok());
        }
        for origin in [
            "http://8.8.8.8",
            "http://192.168.1.2",
            "https://u:p@scanner.example",
            "https://x/path",
            "https://x?token=secret",
        ] {
            assert!(Client::new(origin, &token).is_err());
        }
        assert!(Client::new("http://localhost:8171", "short").is_err());
    }
}
