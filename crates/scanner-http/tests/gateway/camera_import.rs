use crate::support::{Harness, json};
use axum::body::Body;
use base64::Engine as _;
use serde_json::json;

#[tokio::test]
async fn reviewed_import_is_authenticated_idempotent_and_projects_physical_copies() {
    let h = Harness::new();
    let pixels = image::RgbImage::from_fn(300, 420, |x, y| {
        image::Rgb([if x < 140 { 220 } else { 20 }, (y % 250) as u8, 90])
    });
    let mut jpeg = vec![];
    image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
        .encode_image(&pixels)
        .unwrap();
    let id = uuid::Uuid::new_v4();
    let mut body = json!({"id":id,"base64":base64::engine::general_purpose::STANDARD.encode(jpeg),"quantity":2,
        "selection":{"card":{"id":"sv01-104","name":"Greavard","setID":"sv01","setName":"SV","localID":"104","evidence":[],"availableFinishes":["normal"]},"game":"pokemon","language":"en","finish":"normal"}});
    let route = "/api/v1/camera/import";
    assert_eq!(
        h.send(h.request("POST", route, Body::from(body.to_string()), false))
            .await
            .status(),
        401
    );
    assert!(h.engine.database().jobs(10, 0).unwrap().is_empty());
    for _ in 0..2 {
        let response = h
            .send(h.request("POST", route, Body::from(body.to_string()), true))
            .await;
        assert_eq!(response.status(), 200);
        let saved = json(response).await;
        assert_eq!(saved["id"], id.to_string());
        assert_eq!(saved["state"], "completed");
        assert_eq!(saved["images"].as_array().unwrap().len(), 2);
        assert_eq!(saved["cardGames"][1]["frontSequence"], 2);
        assert_eq!(saved["cardIdentifications"][0]["state"], "confirmed");
    }
    let records = h.engine.database().jobs(10, 0).unwrap();
    assert_eq!(records.len(), 1);
    let mut inventory = scanner_library::Inventory::default();
    inventory.add(&records[0]).unwrap();
    let page = inventory.page(50, 0, "", "").unwrap();
    assert_eq!(page.total_copies, 2);
    assert_eq!(page.catalogue_copies, 2);
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].confirmed_quantity, 2);
    let value = scanner_pricing::selection::select(&records[0]).unwrap();
    assert!(value.iter().all(|s| s.matched.is_some()));
    body["quantity"] = json!(3);
    assert_eq!(
        h.send(h.request("POST", route, Body::from(body.to_string()), true))
            .await
            .status(),
        409
    );
    assert_eq!(h.engine.database().jobs(10, 0).unwrap().len(), 1);
    assert_eq!(h.driver.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    h.engine.shutdown().await;
}
