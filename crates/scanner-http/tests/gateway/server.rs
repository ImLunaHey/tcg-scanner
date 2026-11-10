use super::support::Harness;
use scanner_http::{Configuration, Gateway};
use std::{sync::Arc, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn real_tcp_client_uses_rust_rpc_and_listener_stops_without_new_requests() {
    let h = Harness::new();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let gateway = Gateway::new(
        h.engine.clone(),
        "Test",
        &h.token,
        Configuration::new(address, false, None).unwrap(),
    )
    .unwrap();
    let stop = CancellationToken::new();
    let server = tokio::spawn(gateway.serve(listener, stop.clone()));
    let client = scanner_client::Client::new(&format!("http://{address}"), &h.token).unwrap();
    let result = client
        .call("service.status", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(result["queuedJobs"], 0);
    let rpc_failure = client
        .call("profiles.get", serde_json::json!({"id":"unknown"}))
        .await
        .unwrap_err();
    assert_eq!(rpc_failure.kind, "profileNotFound");
    let bad = scanner_client::Client::new(&format!("http://{address}"), &"0".repeat(64)).unwrap();
    assert!(
        bad.call("service.status", serde_json::json!({}))
            .await
            .is_err()
    );
    stop.cancel();
    server.await.unwrap().unwrap();
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
    h.engine.shutdown().await;
}
#[tokio::test]
async fn listener_refuses_a_socket_outside_the_validated_configuration() {
    let h = Harness::new();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gateway = Gateway::new(
        h.engine.clone(),
        "Test",
        &h.token,
        Configuration::new("127.0.0.1:8171".parse().unwrap(), false, None).unwrap(),
    )
    .unwrap();
    assert!(
        gateway
            .serve(listener, CancellationToken::new())
            .await
            .is_err()
    );
    h.engine.shutdown().await;
}
#[tokio::test]
async fn oversized_headers_are_rejected_before_scanner_discovery() {
    let h = Harness::new();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let gateway = Gateway::new(
        Arc::clone(&h.engine),
        "Test",
        &h.token,
        Configuration::new(address, false, None).unwrap(),
    )
    .unwrap();
    let stop = CancellationToken::new();
    let server = tokio::spawn(gateway.serve(listener, stop.clone()));
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    let head = format!(
        "GET /api/v1/devices HTTP/1.1\r\nHost: {address}\r\nX-Oversized: {}\r\n\r\n",
        "x".repeat(40000)
    );
    socket.write_all(head.as_bytes()).await.unwrap();
    let mut response = vec![];
    tokio::time::timeout(Duration::from_secs(2), socket.read_to_end(&mut response))
        .await
        .unwrap()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&response).starts_with("HTTP/1.1 431"),
        "{}",
        String::from_utf8_lossy(&response)
    );
    assert_eq!(h.driver.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    stop.cancel();
    server.await.unwrap().unwrap();
    h.engine.shutdown().await;
}
