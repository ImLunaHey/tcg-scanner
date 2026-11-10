use super::support::{Harness, json as response_json};
use async_trait::async_trait;
use axum::body::{Body, to_bytes};
use scanner_catalogue::{
    JsonTransport,
    library::{ImageTransport, LibraryCatalogue},
};
use scanner_types::{error::Result, scan::ScanState};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;
struct Catalogue;
#[async_trait]
impl JsonTransport for Catalogue {
    async fn get(&self, path: &str, _: usize) -> Result<Vec<u8>> {
        assert_eq!(path, "en/sets/sv09");
        Ok(serde_json::to_vec(&json!({"id":"sv09","releaseDate":"2025-03-28","cards":[
            {"id":"sv09-177","name":"Owned","localId":"177"},
            {"id":"sv09-178","name":"Missing","localId":"178","image":"https://assets.tcgdex.net/en/sv/sv09/178"}
        ]})).unwrap())
    }
}
#[async_trait]
impl ImageTransport for Catalogue {
    async fn get(&self, url: &str, _: usize) -> Result<Vec<u8>> {
        assert_eq!(url, "https://assets.tcgdex.net/en/sv/sv09/178/low.png");
        Ok(scanner_images::encode_png(image::RgbaImage::from_pixel(
            6,
            9,
            image::Rgba([10, 40, 90, 255]),
        ))
        .unwrap()
        .bytes)
    }
}
fn seed(h: &Harness) {
    h.engine
        .configure_library_catalogue(LibraryCatalogue::with_transports(
            Arc::new(Catalogue),
            Arc::new(Catalogue),
        ))
        .unwrap();
    let (job, _) = h.saved();
    let mut record = h.engine.scan_status(job).unwrap();
    record.request.prepare_cards = Some(true);
    record.request.duplex = true;
    let mut back = record.images[0].clone();
    back.sequence = 2;
    record.images.push(back.clone());
    record.processed_images=Some(vec![serde_json::from_value(json!({
        "request":{"id":Uuid::new_v4(),"jobID":job,"sequence":2,"crop":{"x":0,"y":0,"width":1,"height":1},"quarterTurns":0},
        "createdAt":"2026-11-09T12:00:00Z","sourceSHA256":back.sha256,"image":back,
        "cardPreparation":{"crop":{"x":0,"y":0,"width":1,"height":1},"quarterTurns":0,"orientationVerified":true,"orientationBasis":"pokemon-back-pattern","warnings":[],"method":"fixture"}
    })).unwrap()]);
    let identity=serde_json::from_value(json!({
        "request":{"id":Uuid::new_v4(),"jobID":job,"sequence":1,"language":"en"},"sourceSHA256":record.images[0].sha256,
        "state":"confirmed","selectedCardID":"sv09-177","createdAt":"2026-11-09T12:00:00Z","method":"fixture","assessmentSource":"manual-review",
        "evidence":{"lines":[],"candidates":[{"id":"sv09-177","name":"Salamence ex","localID":"177","setID":"sv09","setName":"Journey Together","evidence":[],"availableFinishes":[]}],"warnings":[],"orientation":0}
    })).unwrap();
    record.replace_identification(&identity).unwrap();
    h.engine.database().save(&record).unwrap();
    // An active copy of identical metadata must not count as another physical card.
    record.id = Uuid::new_v4();
    record.request.request_id = record.id;
    record.state = ScanState::Scanning;
    h.engine
        .database()
        .insert(&record, &record.request)
        .unwrap();
}
#[tokio::test]
async fn inventory_filters_keep_totals_and_checklists_subtract_owned_cards() {
    let h = Harness::new();
    seed(&h);
    assert_eq!(
        h.send(h.request("GET", "/api/v1/inventory", Body::empty(), false))
            .await
            .status(),
        401
    );
    let page = response_json(
        h.send(h.request(
            "GET",
            "/api/v1/inventory?search=absent",
            Body::empty(),
            true,
        ))
        .await,
    )
    .await;
    assert_eq!(page["totalCopies"], 1);
    assert_eq!(page["catalogueCopies"], 1);
    assert_eq!(page["matchingEntries"], 0);
    assert!(page["entries"].as_array().unwrap().is_empty());
    let page=response_json(h.call("POST","/rpc",json!({"jsonrpc":"2.0","id":1,"method":"v1.inventory.list","params":{"search":"","limit":1}})).await).await;
    assert_eq!(page["result"]["entries"][0]["confirmedQuantity"], 1);
    let missing = response_json(
        h.send(h.request(
            "GET",
            "/api/v1/inventory/missing?set=journey-together",
            Body::empty(),
            true,
        ))
        .await,
    )
    .await;
    assert_eq!(missing["catalogueCount"], 2);
    assert_eq!(missing["cards"].as_array().unwrap().len(), 1);
    assert_eq!(missing["cards"][0]["id"], "sv09-178");
    let key = missing["cards"][0]["thumbnailKey"].as_str().unwrap();
    let path = format!("/api/v1/inventory/references/{key}/thumbnail");
    let response = h.send(h.request("GET", &path, Body::empty(), false)).await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "image/jpeg");
    let tag = response.headers()["etag"].clone();
    let bytes = to_bytes(response.into_body(), 512 * 1024).await.unwrap();
    assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 6);
    let mut request = h.request("GET", &path, Body::empty(), false);
    request.headers_mut().insert("if-none-match", tag);
    assert_eq!(h.send(request).await.status(), 304);
    assert_eq!(h.driver.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    h.engine.shutdown().await;
}
#[tokio::test]
async fn inventory_rejects_invalid_filters_and_unregistered_references() {
    let h = Harness::new();
    for path in [
        "/api/v1/inventory?limit=101",
        "/api/v1/inventory?offset=-1",
        "/api/v1/inventory?extra=1",
        "/api/v1/inventory/missing",
    ] {
        assert_eq!(
            h.send(h.request("GET", path, Body::empty(), true))
                .await
                .status(),
            400
        );
    }
    let rpc = response_json(
        h.call(
            "POST",
            "/rpc",
            json!({"jsonrpc":"2.0","id":1,"method":"v1.inventory.list","params":{"search":false}}),
        )
        .await,
    )
    .await;
    assert_eq!(rpc["error"]["code"], -32602);
    let path = format!("/api/v1/inventory/references/{}/thumbnail", "0".repeat(64));
    assert_eq!(
        h.send(h.request("GET", &path, Body::empty(), false))
            .await
            .status(),
        404
    );
    let mut request = h.request("GET", &path, Body::empty(), false);
    request
        .headers_mut()
        .insert("origin", "https://untrusted.example".parse().unwrap());
    assert_eq!(h.send(request).await.status(), 403);
    assert_eq!(
        h.send(h.request("POST", &path, Body::empty(), false))
            .await
            .status(),
        401
    );
    h.engine.shutdown().await;
}
