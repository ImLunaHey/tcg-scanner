use super::*;
use crate::distribution::{BINARIES, build, executable_name};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};
fn package(root: &Path) -> PathBuf {
    let bins = root.join("bins");
    let web = root.join("web");
    std::fs::create_dir(&bins).unwrap();
    std::fs::create_dir(&web).unwrap();
    for binary in BINARIES {
        std::fs::write(bins.join(executable_name(binary)), b"synthetic executable").unwrap();
    }
    let mut files = BTreeMap::new();
    for name in [
        "index.html",
        "app.js",
        "app.css",
        "scanner_ui.js",
        "scanner_ui_bg.wasm",
    ] {
        let bytes = if name.ends_with(".wasm") {
            b"\0asm\x01\0\0\0".as_slice()
        } else {
            b"fixture".as_slice()
        };
        std::fs::write(web.join(name), bytes).unwrap();
        files.insert(name.to_owned(), format!("{:x}", Sha256::digest(bytes)));
    }
    std::fs::write(
        web.join("manifest.json"),
        serde_json::to_vec(&scanner_http::web::WebManifest {
            schema_version: 1,
            files,
        })
        .unwrap(),
    )
    .unwrap();
    let package = root.join("package");
    build::assemble(&bins, &web, &package).unwrap();
    package
}
fn request(package: PathBuf, destination: PathBuf, storage: PathBuf) -> Request {
    Request {
        package,
        destination,
        storage,
        required_smb_share: None,
        library_only: true,
        driver_library: None,
        tesseract_library: None,
        trained_data: None,
    }
}
#[test]
fn install_preserves_storage_refuses_overwrite_and_rejects_corrupt_package_before_creation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let package = package(&root);
    let destination = root.join("installed");
    let storage = root.join("scans");
    std::fs::create_dir(&storage).unwrap();
    std::fs::write(storage.join("original"), b"never alter").unwrap();
    run(request(
        package.clone(),
        destination.clone(),
        storage.clone(),
    ))
    .unwrap();
    let c = Configuration::load(&destination.join("service.json")).unwrap();
    let token = scanner_platform::load_credential(&c.token_file).unwrap();
    assert_eq!(token.len(), 64);
    scanner_http::web::WebAssets::load(&destination.join("web")).unwrap();
    assert!(
        run(request(
            package.clone(),
            destination.clone(),
            storage.clone()
        ))
        .is_err()
    );
    assert_eq!(
        scanner_platform::load_credential(&c.token_file).unwrap(),
        token
    );
    assert_eq!(
        std::fs::read(storage.join("original")).unwrap(),
        b"never alter"
    );
    std::fs::write(
        package.join(executable_name("scanner-service")),
        b"synthetic executablE",
    )
    .unwrap();
    let rejected = root.join("rejected");
    assert!(run(request(package, rejected.clone(), storage)).is_err());
    assert!(!rejected.exists());
}
