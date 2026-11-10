use scanner_types::error::{Result, ScannerError};
use serde::Deserialize;
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    #[serde(rename = "baseURL")]
    base_url: String,
    #[serde(rename = "scannerInstanceID")]
    instance: String,
    #[serde(rename = "tokenFile")]
    token_file: PathBuf,
}
pub(crate) fn load(path: Option<&Path>) -> Result<Option<Arc<dyn scanner_staging::Staging>>> {
    let Some(path) = path else { return Ok(None) };
    let configuration: Configuration = serde_json::from_slice(&private_bytes(path, 4096)?)
        .map_err(|_| error("Invalid private Store configuration."))?;
    let token = String::from_utf8(private_bytes(&configuration.token_file, 4096)?)
        .map_err(|_| error("Invalid private Store credential."))?;
    // v1 token files may contain a final newline; internal whitespace is invalid.
    let client = scanner_staging::Client::new(
        &configuration.base_url,
        configuration.instance,
        token.trim(),
    )?;
    Ok(Some(Arc::new(client)))
}
fn private_bytes(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let directory = path
        .parent()
        .ok_or_else(|| error("Store state requires an absolute private file path."))?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| error("Invalid Store state filename."))?;
    let file = scanner_platform::PrivateDirectory::open(directory, false)?
        .open_file(name, false, false, false)?;
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error("Could not read private Store state."))?;
    if bytes.len() as u64 > maximum {
        return Err(error("Private Store state exceeds its size limit."));
    }
    Ok(bytes)
}
fn error(message: &str) -> ScannerError {
    ScannerError::new("storeConfiguration", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    const ORIGIN: &str = "https://store.example";
    #[test]
    fn private_v1_configuration_loads_with_bounded_credentials() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().canonicalize().unwrap().join("private");
        let directory = scanner_platform::PrivateDirectory::open(&path, true).unwrap();
        let token = path.join("token");
        directory
            .open_file("token", true, true, false)
            .unwrap()
            .write_all(format!("{}\n", "x".repeat(1024)).as_bytes())
            .unwrap();
        let config =
            serde_json::json!({"baseURL":ORIGIN,"scannerInstanceID":"fixture","tokenFile":token});
        directory
            .open_file("config.json", true, true, false)
            .unwrap()
            .write_all(&serde_json::to_vec(&config).unwrap())
            .unwrap();
        let client = load(Some(&path.join("config.json"))).unwrap().unwrap();
        assert_eq!(client.origin(), ORIGIN);
        assert_eq!(client.scanner_instance_id(), "fixture");
        assert!(load(None).unwrap().is_none());
        assert!(load(Some(Path::new("relative/config.json"))).is_err());
        let mut file = directory.open_file("token", true, false, false).unwrap();
        file.set_len(0).unwrap();
        file.write_all(&vec![b'x'; 4097]).unwrap();
        assert!(load(Some(&path.join("config.json"))).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("config.json", path.join("link")).unwrap();
            assert!(load(Some(&path.join("link"))).is_err());
        }
    }
}
