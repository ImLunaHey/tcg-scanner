use scanner_types::error::{Result, ScannerError};
use std::{
    collections::HashSet,
    net::{IpAddr, SocketAddr},
};

#[derive(Clone)]
pub struct Configuration {
    pub(crate) address: SocketAddr,
    pub(crate) origin: String,
    pub(crate) authorities: HashSet<String>,
}
impl Configuration {
    pub fn new(
        address: SocketAddr,
        allow_remote: bool,
        public_origin: Option<&str>,
    ) -> Result<Self> {
        let loopback = matches!(address.ip(), IpAddr::V4(v) if v.octets() == [127,0,0,1])
            || address.ip() == IpAddr::V6(std::net::Ipv6Addr::LOCALHOST);
        let tailscale = matches!(address.ip(), IpAddr::V4(v) if v.octets()[0] == 100 && (64..=127).contains(&v.octets()[1]));
        if address.port() < 1024 || !(loopback || allow_remote && tailscale) {
            return Err(invalid(
                "Bind to loopback or explicitly allowed Tailscale IPv4, using port 1024–65535.",
            ));
        }
        let origin = public_origin
            .map(str::to_owned)
            .unwrap_or_else(|| format!("http://{address}"));
        let parsed = url::Url::parse(&origin).map_err(|_| invalid("Invalid public origin."))?;
        if parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || parsed.path() != "/"
            || parsed.scheme()
                != if public_origin.is_some() {
                    "https"
                } else {
                    "http"
                }
            || origin.ends_with('/')
            || origin.contains(['\r', '\n', '\\'])
        {
            return Err(invalid(
                "Public origin must be HTTPS with no credentials, path, query or fragment.",
            ));
        }
        let authority = origin
            .split_once("://")
            .ok_or_else(|| invalid("Invalid origin."))?
            .1;
        if authority.contains('/') || origin.chars().any(char::is_whitespace) {
            return Err(invalid(
                "Public origin must contain only a scheme and authority.",
            ));
        }
        let mut authorities = HashSet::from([address.to_string(), authority.to_ascii_lowercase()]);
        if loopback {
            authorities.insert(format!("localhost:{}", address.port()));
        }
        Ok(Self {
            address,
            origin,
            authorities,
        })
    }
    pub fn address(&self) -> SocketAddr {
        self.address
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
}
fn invalid(message: &str) -> ScannerError {
    ScannerError::new("configuration", message)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requires_explicit_private_binds_and_bare_https_origins() {
        for address in [
            "0.0.0.0:8170",
            "192.168.1.2:8170",
            "8.8.8.8:8170",
            "127.0.0.1:80",
        ] {
            assert!(Configuration::new(address.parse().unwrap(), true, None).is_err());
        }
        let ip = "100.64.0.1:8170".parse().unwrap();
        assert!(Configuration::new(ip, false, None).is_err());
        assert!(Configuration::new(ip, true, Some("https://scanner.example")).is_ok());
        for origin in [
            "http://scanner.example",
            "https://x/",
            "https://u:p@x",
            "https://x/path",
            "https://x?q=1",
            "https://x#f",
        ] {
            assert!(Configuration::new(ip, true, Some(origin)).is_err());
        }
    }
}
