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
    pub catalogue_cache: Option<PathBuf>,
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
        value.storage.validate()?;
        Ok(value)
    }
}
pub(crate) fn config(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("configuration", error.to_string())
}
