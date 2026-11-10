use scanner_files::configuration::StorageConfiguration;
use scanner_types::error::{Result, ScannerError};
use serde::Deserialize;
use std::{
    io::Read,
    net::SocketAddr,
    path::{Path, PathBuf},
};

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Configuration {
    pub database: PathBuf,
    pub store_configuration: Option<PathBuf>,
    pub catalogue_cache: Option<PathBuf>,
    pub web_directory: Option<PathBuf>,
    pub storage: StorageConfiguration,
    pub token_file: PathBuf,
    #[serde(default = "listen")]
    pub listen: SocketAddr,
    #[serde(default)]
    pub allow_remote: bool,
    pub public_origin: Option<String>,
    #[serde(default)]
    pub backend: Backend,
    pub driver_library: Option<PathBuf>,
    pub scan_button_device: Option<String>,
    pub tesseract_library: Option<PathBuf>,
    pub trained_data: Option<PathBuf>,
}
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Backend {
    #[default]
    Native,
    LibraryOnly,
}
fn listen() -> SocketAddr {
    "127.0.0.1:8171".parse().unwrap()
}
impl Configuration {
    pub fn load(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path).map_err(config)?;
        let mut bytes = vec![];
        file.take(65537).read_to_end(&mut bytes).map_err(config)?;
        if bytes.len() > 65536 {
            return Err(config("Configuration exceeds 64 KiB."));
        }
        let value: Self = serde_json::from_slice(&bytes).map_err(config)?;
        if !value.database.is_absolute() || !value.token_file.is_absolute() {
            return Err(config("Database and credential paths must be absolute."));
        }
        if value
            .catalogue_cache
            .as_ref()
            .is_some_and(|p| !p.is_absolute() || p.file_name().is_none())
        {
            return Err(config("Catalogue cache must be an absolute filename."));
        }
        if value
            .web_directory
            .as_ref()
            .is_some_and(|p| !p.is_absolute() || !p.is_dir())
        {
            return Err(config(
                "webDirectory must be an existing absolute directory.",
            ));
        }
        if value.tesseract_library.is_some() != value.trained_data.is_some() {
            return Err(config("Configure both tesseractLibrary and trainedData."));
        }
        for path in [
            &value.driver_library,
            &value.tesseract_library,
            &value.trained_data,
        ]
        .into_iter()
        .flatten()
        {
            if !path.is_absolute() || !path.exists() {
                return Err(config(
                    "Native library and data paths must be absolute and exist.",
                ));
            }
        }
        if let Some(device) = &value.scan_button_device
            && (device.is_empty()
                || device.len() > 512
                || device.contains('\0')
                || matches!(value.backend, Backend::LibraryOnly))
        {
            return Err(config(
                "Scan-button device must identify a native scanner (1–512 bytes).",
            ));
        }
        value.storage.validate_shape()?;
        Ok(value)
    }
}
pub(crate) fn config(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("configuration", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offline_storage_does_not_prevent_loading_valid_service_configuration() {
        let temporary = tempfile::tempdir().unwrap();
        let missing = temporary.path().join("unmounted");
        let path = temporary.path().join("service.json");
        let mut config = serde_json::json!({
            "database": temporary.path().join("local/state.sqlite"),
            "tokenFile": temporary.path().join("private/token"),
            "backend":"libraryOnly",
            "storage":{"storageRoot": missing, "requiredSMBShare":"smb://lake/scans"}
        });
        std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
        let loaded = Configuration::load(&path).unwrap();
        assert!(loaded.storage.validate().is_err());
        assert!(!missing.exists());
        config["storage"]["requiredSMBShare"] = serde_json::json!("smb://user:secret@lake/scans");
        std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
        assert!(Configuration::load(&path).is_err());
    }
}
