use super::support::Harness;
use axum::body::{Body, to_bytes};
use serde_json::json;
#[tokio::test]
async fn public_binary_downloads_hold_slots_and_verify_the_complete_source() {
    let h = Harness::new();
    let (job, png) = h.saved();
    let path = format!("/api/v1/images/{job}:1");
    let first = h.send(h.request("GET", &path, Body::empty(), false)).await;
    let second = h.send(h.request("GET", &path, Body::empty(), false)).await;
    assert_eq!(first.status(), 200);
    assert_eq!(second.status(), 200);
    let tag = first.headers()["etag"].clone();
    assert_eq!(
        h.send(h.request("GET", &path, Body::empty(), false))
            .await
            .status(),
        429
    );
    assert_eq!(
        to_bytes(first.into_body(), 1024).await.unwrap().as_ref(),
        png
    );
    drop(second);
    let mut request = h.request("GET", &path, Body::empty(), false);
    request.headers_mut().insert("if-none-match", tag);
    assert_eq!(h.send(request).await.status(), 304);
    let thumb = h
        .send(h.request(
            "GET",
            &format!("{path}/thumbnail?format=jpeg"),
            Body::empty(),
            false,
        ))
        .await;
    assert_eq!(thumb.status(), 200);
    assert_eq!(thumb.headers()["content-type"], "image/jpeg");
    drop(thumb);
    let record = h.engine.scan_status(job).unwrap();
    let location =
        std::path::Path::new(&record.output_directory).join(&record.images[0].relative_path);
    let mut changed = png;
    changed[0] ^= 1;
    std::fs::write(location, changed).unwrap();
    let changed = h.send(h.request("GET", &path, Body::empty(), false)).await;
    assert!(to_bytes(changed.into_body(), 1024).await.is_err());
    h.engine.shutdown().await;
}
#[tokio::test]
async fn event_streams_have_separate_limits_and_explicit_resume_cursors() {
    let h = Harness::new();
    h.engine
        .database()
        .event("custom\nevent: injected", &json!({"message":"line\nbreak"}))
        .unwrap();
    let mut responses = vec![];
    for _ in 0..4 {
        let r = h
            .send(h.request(
                "GET",
                "/api/v1/events?after=0&seconds=1",
                Body::empty(),
                true,
            ))
            .await;
        assert_eq!(r.status(), 200);
        responses.push(r);
    }
    assert_eq!(
        h.send(h.request("GET", "/api/v1/events", Body::empty(), true))
            .await
            .status(),
        429
    );
    let first = responses.pop().unwrap();
    let text =
        String::from_utf8(to_bytes(first.into_body(), 65536).await.unwrap().to_vec()).unwrap();
    assert!(text.contains("event: scanner\n"));
    assert!(!text.contains("\nevent: injected\n"));
    let live = h
        .send(h.request("GET", "/api/v1/events?seconds=1", Body::empty(), true))
        .await;
    let text =
        String::from_utf8(to_bytes(live.into_body(), 65536).await.unwrap().to_vec()).unwrap();
    assert!(text.contains("event: ready\n"));
    assert!(!text.contains("event: scanner\n"));
    drop(responses);
    h.engine.shutdown().await;
}
