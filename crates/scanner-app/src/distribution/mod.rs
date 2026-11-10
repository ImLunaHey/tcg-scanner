//! Distribution integrity and bounded copying, shared by packaging and setup.
pub mod build;
mod read;
use crate::configuration::config;
use scanner_types::error::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Component, Path},
};

pub const BINARIES: &[&str] = &[
    "scanner-service",
    "scanner",
    "scanner-mcp",
    "scanner-index",
    "scanner-setup",
    "scanner-ui",
];
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub version: String,
    pub os: String,
    pub architecture: String,
    pub files: BTreeMap<String, Entry>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub bytes: u64,
    pub sha256: String,
    pub executable: bool,
}
pub struct Distribution {
    root: cap_std::fs::Dir,
    pub manifest: Manifest,
}
impl Distribution {
    pub fn load(path: &Path) -> Result<Self> {
        let root = cap_std::fs::Dir::open_ambient_dir(path, cap_std::ambient_authority())
            .map_err(config)?;
        let manifest: Manifest =
            serde_json::from_slice(&read::bytes(&root, "distribution.json", 65536)?)
                .map_err(config)?;
        if manifest.schema_version != 1
            || manifest.os != std::env::consts::OS
            || manifest.architecture != std::env::consts::ARCH
            || !(6..=128).contains(&manifest.files.len())
        {
            return Err(config(
                "Distribution version, platform or file count is invalid.",
            ));
        }
        let mut total = 0u64;
        for (name, entry) in &manifest.files {
            validate_name(name)?;
            total = total
                .checked_add(entry.bytes)
                .ok_or_else(|| config("Distribution too large."))?;
            if entry.bytes > 512 * 1024 * 1024
                || total > 1024 * 1024 * 1024
                || entry.sha256.len() != 64
            {
                return Err(config("Distribution size or checksum is invalid."));
            }
        }
        for binary in BINARIES {
            if !manifest
                .files
                .get(&executable_name(binary))
                .is_some_and(|e| e.executable)
            {
                return Err(config("Distribution is missing an application executable."));
            }
        }
        if !manifest.files.contains_key("web/manifest.json") {
            return Err(config("Missing browser bundle."));
        }
        Ok(Self { root, manifest })
    }
    pub fn read(&self, name: &str) -> Result<Vec<u8>> {
        use sha2::{Digest, Sha256};
        let entry = self
            .manifest
            .files
            .get(name)
            .ok_or_else(|| config("Unknown distribution file."))?;
        let bytes = read::bytes(&self.root, name, entry.bytes)?;
        if bytes.len() as u64 != entry.bytes
            || format!("{:x}", Sha256::digest(&bytes)) != entry.sha256
        {
            return Err(config(format!("Distribution integrity failed for {name}.")));
        }
        Ok(bytes)
    }
}
pub fn executable_name(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}
fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.contains(['\\', ':', '\0'])
        || name.len() > 256
        || Path::new(name)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || Path::new(name).components().count() > 8
    {
        return Err(config("Invalid distribution filename."));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distribution_names_cannot_escape_or_use_windows_device_syntax() {
        for name in ["../token", "/token", "C:/token", "file:stream", "a\\b", ""] {
            assert!(validate_name(name).is_err());
        }
        assert!(validate_name("web/icons/icon.png").is_ok());
    }
}
