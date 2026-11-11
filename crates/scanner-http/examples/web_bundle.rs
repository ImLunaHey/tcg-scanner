//! Development packaging only: add the HTML bootstrap and hash wasm-bindgen output.
#[path = "web_bundle/camera.rs"]
mod camera;
use scanner_http::web::{WebAssets, WebManifest, file_type};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, error::Error, path::Path};
fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or("Usage: web_bundle WASM_BINDGEN_OUTPUT_DIRECTORY")?;
    let output = Path::new(&output).canonicalize()?;
    let existing = std::env::args_os()
        .nth(2)
        .is_some_and(|arg| arg == "--existing");
    if !existing {
        write_rune_bootstrap(&output)?;
    }
    let mut files = BTreeMap::new();
    collect(&output, &output, &mut files)?;
    let manifest = WebManifest {
        schema_version: 1,
        files,
    };
    std::fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    WebAssets::load(&output)?;
    println!(
        "Verified {} browser assets in {}",
        manifest.files.len(),
        output.display()
    );
    Ok(())
}
fn write_rune_bootstrap(output: &Path) -> Result<(), Box<dyn Error>> {
    let templates = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scanner-ui/web");
    for name in [
        "index.html",
        "app.js",
        "app.css",
        "Inter-Regular.ttf",
        "Inter-Medium.ttf",
        "Inter-SemiBold.ttf",
        "Inter-LICENSE.txt",
    ] {
        std::fs::copy(templates.join(name), output.join(name))?;
    }
    std::fs::copy(
        templates.join("../assets/NotoSansKR-LICENSE.txt"),
        output.join("NotoSansKR-LICENSE.txt"),
    )?;
    camera::write(output)?;
    Ok(())
}
fn collect(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, String>,
) -> Result<(), Box<dyn Error>> {
    if directory.strip_prefix(root)?.components().count() > 12 {
        return Err("Too many nested directories".into());
    }
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err("Browser bundles must not contain symbolic links".into());
        }
        if kind.is_dir() {
            collect(root, &path, files)?;
            continue;
        }
        let name = path
            .strip_prefix(root)?
            .to_str()
            .ok_or("Invalid UTF-8 asset path")?
            .replace('\\', "/");
        if name == "manifest.json" || name.ends_with(".d.ts") {
            continue;
        }
        let (_, limit) = file_type(&name)?;
        if entry.metadata()?.len() > limit as u64 || files.len() >= 64 {
            return Err("Browser bundle exceeds limits".into());
        }
        let bytes = std::fs::read(path)?;
        files.insert(name, format!("{:x}", Sha256::digest(bytes)));
    }
    Ok(())
}
