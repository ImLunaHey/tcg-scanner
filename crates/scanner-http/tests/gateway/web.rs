use super::support::Harness;
use axum::{
    body::{Body, to_bytes},
    http::header,
};
use scanner_http::web::{WebAssets, WebManifest};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};
use tower::ServiceExt;

fn bundle(root: &Path) {
    let mut files = BTreeMap::new();
    for (name, bytes) in [
        ("index.html", b"<p>Scanner</p>".as_slice()),
        ("app.js", b"// app"),
        ("app.css", b"body {}"),
        ("scanner_ui.js", b"// generated"),
        ("scanner_ui_bg.wasm", b"\0asm\x01\0\0\0"),
    ] {
        std::fs::write(root.join(name), bytes).unwrap();
        files.insert(name.into(), format!("{:x}", Sha256::digest(bytes)));
    }
    std::fs::write(
        root.join("manifest.json"),
        serde_json::to_vec(&WebManifest {
            schema_version: 1,
            files,
        })
        .unwrap(),
    )
    .unwrap();
}
#[tokio::test]
async fn verified_browser_assets_are_public_but_do_not_bypass_api_or_origin_checks() {
    let h = Harness::new();
    let directory = tempfile::tempdir().unwrap();
    bundle(directory.path());
    h.gateway
        .configure_web_assets(WebAssets::load(directory.path()).unwrap())
        .unwrap();
    assert!(
        h.gateway
            .configure_web_assets(WebAssets::load(directory.path()).unwrap())
            .is_err()
    );
    // The running gateway serves the verified immutable bytes, not a mutable disk lookup.
    std::fs::write(
        directory.path().join("index.html"),
        b"changed after loading",
    )
    .unwrap();
    let response = h
        .app
        .clone()
        .oneshot(h.request("GET", "/", Body::empty(), false))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .contains("wasm-unsafe-eval")
    );
    let etag = response.headers()[header::ETAG].clone();
    assert_eq!(
        &to_bytes(response.into_body(), 1000).await.unwrap()[..],
        b"<p>Scanner</p>"
    );
    let mut request = h.request("GET", "/", Body::empty(), false);
    request.headers_mut().insert(header::IF_NONE_MATCH, etag);
    assert_eq!(h.app.clone().oneshot(request).await.unwrap().status(), 304);
    let response = h
        .app
        .clone()
        .oneshot(h.request("HEAD", "/scanner_ui_bg.wasm", Body::empty(), false))
        .await
        .unwrap();
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/wasm");
    assert_eq!(response.headers()[header::CONTENT_LENGTH], "8");
    assert!(
        to_bytes(response.into_body(), 100)
            .await
            .unwrap()
            .is_empty()
    );
    for (method, target, expected) in [
        ("GET", "/camera", 200),
        ("GET", "/api/v1/jobs", 401),
        ("POST", "/app.js", 401),
        ("GET", "/app.js?token=hidden", 400),
    ] {
        assert_eq!(
            h.app
                .clone()
                .oneshot(h.request(method, target, Body::empty(), false))
                .await
                .unwrap()
                .status(),
            expected
        );
    }
    for (header, value) in [
        (header::ORIGIN, "https://untrusted.example"),
        (header::HOST, "untrusted.example"),
    ] {
        let mut request = h.request("GET", "/", Body::empty(), false);
        request.headers_mut().insert(header, value.parse().unwrap());
        assert_eq!(h.app.clone().oneshot(request).await.unwrap().status(), 403);
    }
    h.engine.shutdown().await;
}
#[test]
fn corrupted_incomplete_linked_or_oversized_bundles_are_rejected_before_serving() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    bundle(root);
    std::fs::write(root.join("app.js"), "tampered").unwrap();
    assert!(WebAssets::load(root).is_err());
    bundle(root);
    std::fs::remove_file(root.join("app.js")).unwrap();
    assert!(WebAssets::load(root).is_err());
    bundle(root);
    std::fs::OpenOptions::new()
        .write(true)
        .open(root.join("scanner_ui_bg.wasm"))
        .unwrap()
        .set_len(65 * 1024 * 1024)
        .unwrap();
    assert!(WebAssets::load(root).is_err());
    #[cfg(unix)]
    {
        bundle(root);
        std::fs::remove_file(root.join("app.js")).unwrap();
        std::os::unix::fs::symlink(root.join("scanner_ui.js"), root.join("app.js")).unwrap();
        assert!(WebAssets::load(root).is_err());
    }
    for name in [
        "../secret.js",
        "x//file.js",
        "/file.js",
        "x\\file.js",
        "token.txt",
        "other.html",
    ] {
        assert!(scanner_http::web::file_type(name).is_err());
    }
}
