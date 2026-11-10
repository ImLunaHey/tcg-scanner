use super::support::{Harness, json as response_json};
use axum::body::Body;
use scanner_types::identification::{CardIdentification, IdentificationState, ReviewRequest};
use serde_json::json;
use uuid::Uuid;
fn seed(h: &Harness, job: Uuid) -> Uuid {
    let mut record = h.engine.scan_status(job).unwrap();
    let id = Uuid::new_v4();
    let result:CardIdentification=serde_json::from_value(json!({
        "request":{"id":id,"jobID":job,"sequence":1,"language":"en","futureInput":7},
        "sourceSHA256":record.images[0].sha256,"state":"reviewRequired","createdAt":"2026-11-09T12:00:00Z",
        "method":"saved-reference","assessmentSource":"manual-review",
        "evidence":{"lines":[],"candidates":[{"id":"sv09-177","name":"Salamence ex","localID":"177","setID":"sv09","setName":"Journey Together","evidence":["Collector number 177"],"availableFinishes":["holo"],"futureCandidate":true}],"warnings":[],"orientation":0,"raritySymbol":{"future":7}},
        "finishEstimate":{"edition":{"stamp":"preserved"}},"conditionCheck":{"regions":[1,2]},"futureAssessment":{"value":9}
    })).unwrap();
    record.replace_identification(&result).unwrap();
    h.engine.database().save(&record).unwrap();
    id
}
#[tokio::test]
async fn visual_artwork_requires_explicit_review_before_becoming_an_identity() {
    let h = Harness::new();
    let (job, bytes) = h.saved();
    let id = seed(&h, job);
    let mut record = h.engine.scan_status(job).unwrap();
    let mut identity = record.identifications().unwrap().remove(0);
    let evidence = identity.evidence.as_mut().unwrap();
    let candidate = evidence.candidates.remove(0);
    evidence.extensions.insert(
        "visualMatches".into(),
        json!([{"similarity":0.91,"candidate":candidate}]),
    );
    identity.state = IdentificationState::NoMatch;
    record.replace_identification(&identity).unwrap();
    h.engine.database().save(&record).unwrap();
    let path = format!("/api/v1/images/{job}:1/identification");
    let before = response_json(h.send(h.request("GET", &path, Body::empty(), true)).await).await;
    assert_eq!(
        before["identification"]["evidence"]["candidates"],
        json!([])
    );
    let request = json!({"identificationID":id,"cardID":"sv09-177"});
    assert_eq!(
        h.send(h.request("PATCH", &path, Body::from(request.to_string()), false))
            .await
            .status(),
        401
    );
    let confirmed = response_json(h.call("PATCH", &path, request).await).await;
    assert_eq!(confirmed["state"], "confirmed");
    assert_eq!(confirmed["selectedCardID"], "sv09-177");
    assert_eq!(
        confirmed["evidence"]["candidates"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(confirmed["finish"], "unverified");
    assert_eq!(confirmed["condition"], "unassessed");
    let record = h.engine.scan_status(job).unwrap();
    assert_eq!(
        h.engine
            .storage()
            .read_image(&record, &record.images[0], 1024)
            .unwrap(),
        bytes
    );
    assert_eq!(h.driver.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    h.engine.shutdown().await;
}
#[tokio::test]
async fn condition_route_requires_authentication_and_never_checks_an_unknown_game() {
    let h = Harness::new();
    let (job, _) = h.saved();
    let id = seed(&h, job);
    for operation in ["condition-check", "finish-estimate"] {
        let path = format!("/api/v1/images/{job}:1/{operation}");
        let request = json!({"identificationID":id});
        let denied = h
            .send(h.request("POST", &path, Body::from(request.to_string()), false))
            .await;
        assert_eq!(denied.status(), 401);
        let response = response_json(h.call("POST", &path, request).await).await;
        assert_eq!(response["error"]["kind"], "unsupportedGame");
        let invalid = h
            .call(
                "POST",
                &path,
                json!({"identificationID":id,"path":"untrusted"}),
            )
            .await;
        assert_eq!(invalid.status(), 400);
    }
    h.engine.shutdown().await;
}
#[tokio::test]
async fn authenticated_rest_reviews_share_rpc_state_and_preserve_originals_and_evidence() {
    let h = Harness::new();
    let (job, bytes) = h.saved();
    let id = seed(&h, job);
    let path = format!("/api/v1/images/{job}:1/identification");
    let review = json!({"identificationID":id,"cardID":"sv09-177","finish":"holo","condition":"lightlyPlayed"});
    let unauthorized = h
        .send(h.request("PATCH", &path, Body::from(review.to_string()), false))
        .await;
    assert_eq!(unauthorized.status(), 401);
    let response = h.call("PATCH", &path, review).await;
    assert_eq!(response.status(), 200);
    let reviewed = response_json(response).await;
    assert_eq!(reviewed["state"], "confirmed");
    assert_eq!(reviewed["condition"], "lightlyPlayed");
    assert_eq!(reviewed["finishEstimate"]["edition"]["stamp"], "preserved");
    assert_eq!(reviewed["request"]["futureInput"], 7);
    assert_eq!(
        reviewed["evidence"]["candidates"][0]["futureCandidate"],
        true
    );
    let rpc=h.call("POST","/rpc",json!({"jsonrpc":"2.0","id":1,"method":"v1.images.identification","params":{"jobID":job,"sequence":1}})).await;
    assert_eq!(
        response_json(rpc).await["result"]["identification"],
        reviewed
    );
    let record = h.engine.scan_status(job).unwrap();
    assert_eq!(
        h.engine
            .storage()
            .read_image(&record, &record.images[0], 1024)
            .unwrap(),
        bytes
    );
    assert_eq!(
        h.engine
            .storage()
            .read_manifest(&record)
            .unwrap()
            .identifications()
            .unwrap()[0]
            .state,
        IdentificationState::Confirmed
    );
    let rejected =
        response_json(h.call("PATCH", &path, json!({"identificationID":id})).await).await;
    assert_eq!(rejected["state"], "rejected");
    assert!(rejected.get("selectedCardID").is_none());
    assert_eq!(rejected["finish"], "unverified");
    assert_eq!(rejected["condition"], "unassessed");
    assert_eq!(h.driver.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    h.engine.shutdown().await;
}
#[tokio::test]
async fn stale_invalid_and_inflight_reviews_cannot_overwrite_the_identification() {
    let h = Harness::new();
    let (job, _) = h.saved();
    let id = seed(&h, job);
    let request = |identification_id, card_id| ReviewRequest {
        job_id: job,
        sequence: 1,
        identification_id,
        card_id,
        finish: Default::default(),
        condition: Default::default(),
    };
    assert_eq!(
        h.engine
            .review_card(request(Uuid::new_v4(), None))
            .await
            .unwrap_err()
            .kind,
        "conflict"
    );
    assert_eq!(
        h.engine
            .review_card(request(id, Some("invented".into())))
            .await
            .unwrap_err()
            .kind,
        "invalidParams"
    );
    let mut record = h.engine.scan_status(job).unwrap();
    let mut identification = record.identifications().unwrap().remove(0);
    identification.state = IdentificationState::Processing;
    record.replace_identification(&identification).unwrap();
    h.engine.database().save(&record).unwrap();
    assert_eq!(
        h.engine
            .review_card(request(id, None))
            .await
            .unwrap_err()
            .kind,
        "busy"
    );
    assert_eq!(
        h.engine
            .card_identification(job, 1)
            .await
            .unwrap()
            .unwrap()
            .state,
        IdentificationState::Processing
    );
    h.engine.shutdown().await;
    assert_eq!(
        h.engine
            .review_card(request(id, None))
            .await
            .unwrap_err()
            .kind,
        "busy"
    );
}

#[tokio::test]
async fn valuation_read_is_authenticated_strict_and_does_not_price_unknown_pairs() {
    let h = Harness::new();
    let (job, _) = h.saved();
    seed(&h, job);
    let path = format!("/api/v1/batches/{job}/valuation");
    assert_eq!(
        h.send(h.request("GET", &path, Body::empty(), false))
            .await
            .status(),
        401
    );
    let response = response_json(h.send(h.request("GET", &path, Body::empty(), true)).await).await;
    assert_eq!(response["currency"], "GBP", "{response}");
    assert_eq!(response["pricedCount"], 0);
    assert!(response.get("lowPence").is_none());
    assert_eq!(response["refreshing"], false);
    assert_eq!(
        h.send(h.request("GET", &format!("{path}?currency=USD"), Body::empty(), true))
            .await
            .status(),
        400
    );
    h.engine.shutdown().await;
}
