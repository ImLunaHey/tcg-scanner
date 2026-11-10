//! Installed native UI reads only its adjacent private service configuration.
use scanner_types::error::{Result, ScannerError};
use std::{io::Read, net::SocketAddr, path::PathBuf};
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Connection {
    token_file: PathBuf,
    listen: SocketAddr,
}
pub fn installed() -> Result<Option<(String, PathBuf)>> {
    let error = |e: std::io::Error| ScannerError::new("configuration", e.to_string());
    let executable = std::env::current_exe().map_err(error)?;
    let Some(parent) = executable.parent() else {
        return Ok(None);
    };
    if !parent.join("service.json").exists() {
        return Ok(None);
    }
    let directory = scanner_platform::PrivateDirectory::open(parent, false)?;
    let mut bytes = Vec::new();
    directory
        .open_file("service.json", false, false, false)?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() > 65536 {
        return Err(ScannerError::new(
            "configuration",
            "Service configuration exceeds 64 KiB.",
        ));
    }
    let mut c: Connection = serde_json::from_slice(&bytes)
        .map_err(|e| ScannerError::new("configuration", e.to_string()))?;
    if c.listen.ip().is_unspecified() {
        c.listen.set_ip(if c.listen.is_ipv6() {
            std::net::Ipv6Addr::LOCALHOST.into()
        } else {
            std::net::Ipv4Addr::LOCALHOST.into()
        });
    }
    Ok(Some((format!("http://{}", c.listen), c.token_file)))
}
