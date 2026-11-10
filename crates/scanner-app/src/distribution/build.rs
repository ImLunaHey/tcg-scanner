//! Development-only artifact assembly; compilers run separately in CI.
use super::{BINARIES, Entry, Manifest, config, executable_name};
use scanner_types::error::Result;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

pub fn assemble(binaries: &Path, web: &Path, output: &Path) -> Result<()> {
    scanner_http::web::WebAssets::load(web)?;
    std::fs::create_dir(output).map_err(config)?;
    let mut files = BTreeMap::new();
    for binary in BINARIES {
        let name = executable_name(binary);
        add(&binaries.join(&name), output, &name, true, &mut files)?;
    }
    let manifest: scanner_http::web::WebManifest =
        serde_json::from_slice(&std::fs::read(web.join("manifest.json")).map_err(config)?)
            .map_err(config)?;
    for name in manifest
        .files
        .keys()
        .chain(std::iter::once(&"manifest.json".to_string()))
    {
        add(
            &web.join(name),
            output,
            &format!("web/{name}"),
            false,
            &mut files,
        )?;
    }
    for (name, contents) in [
        (
            "INSTALL.md",
            include_str!("../../../../Docs/portable-install.md"),
        ),
        (
            "Rune-LICENSE.txt",
            include_str!("../../../../vendor/rune/LICENSE"),
        ),
    ] {
        std::fs::write(output.join(name), contents).map_err(config)?;
        files.insert(
            name.into(),
            Entry {
                bytes: contents.len() as u64,
                sha256: format!("{:x}", Sha256::digest(contents.as_bytes())),
                executable: false,
            },
        );
    }
    let manifest = Manifest {
        schema_version: 1,
        version: env!("CARGO_PKG_VERSION").into(),
        os: std::env::consts::OS.into(),
        architecture: std::env::consts::ARCH.into(),
        files,
    };
    std::fs::write(
        output.join("distribution.json"),
        serde_json::to_vec_pretty(&manifest).map_err(config)?,
    )
    .map_err(config)?;
    let verified = super::Distribution::load(output)?;
    for name in verified.manifest.files.keys() {
        verified.read(name)?;
    }
    Ok(())
}
fn add(
    source: &Path,
    output: &Path,
    name: &str,
    executable: bool,
    files: &mut BTreeMap<String, Entry>,
) -> Result<()> {
    super::validate_name(name)?;
    let metadata = std::fs::symlink_metadata(source).map_err(config)?;
    if !metadata.is_file() || metadata.len() > 512 * 1024 * 1024 {
        return Err(config("Invalid package source."));
    }
    let bytes = std::fs::read(source).map_err(config)?;
    let target = output.join(name);
    std::fs::create_dir_all(target.parent().unwrap()).map_err(config)?;
    std::fs::copy(source, &target).map_err(config)?;
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))
            .map_err(config)?;
    }
    files.insert(
        name.into(),
        Entry {
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            executable,
        },
    );
    Ok(())
}
