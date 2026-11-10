#![cfg(not(target_arch = "wasm32"))]
use base64::{Engine, engine::general_purpose::STANDARD};
use scanner_client::Client;
use scanner_export::Batch;
use scanner_types::{images::OriginalImage, scan::ScanRecord};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
use uuid::Uuid;

struct Fixture {
    client: Client,
    record: ScanRecord,
    bytes: Vec<u8>,
    server: JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
impl Fixture {
    async fn new(corrupt_edit: bool) -> Self {
        let bytes = vec![42; 300_000];
        let mut record = ScanRecord::new(Default::default(), Default::default(), "/source".into());
        record.batch_number = Some(1);
        let image:OriginalImage=serde_json::from_value(json!({"sequence":1,"relativePath":"../../outside.png","bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes)),"width":100,"height":150,"imageCount":1,"uniformType":"public.png","side":"unknown"})).unwrap();
        record.images.push(image.clone());
        record.processed_images=Some(vec![serde_json::from_value(json!({"request":{"id":Uuid::new_v4(),"jobID":record.id,"sequence":1,"crop":{"x":0,"y":0,"width":1,"height":1},"quarterTurns":0},"createdAt":"2026-11-10T00:00:00Z","sourceSHA256":image.sha256,"image":image})).unwrap()]);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = Client::new(
            &format!("http://{}", listener.local_addr().unwrap()),
            &"a".repeat(64),
        )
        .unwrap();
        let source = record.clone();
        let data = bytes.clone();
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let request = request(&mut socket).await;
                let p = &request["params"];
                let metadata =
                    json!({"schemaVersion":1,"jobID":source.id,"image":source.images[0]});
                let result = if request["method"] == "v1.images.get" {
                    metadata
                } else {
                    assert_eq!(request["method"], "v1.images.read");
                    let offset = p["offset"].as_u64().unwrap() as usize;
                    let end = (offset + p["length"].as_u64().unwrap() as usize).min(data.len());
                    let mut bytes = data[offset..end].to_vec();
                    if corrupt_edit && p.get("editID").is_some() {
                        bytes[0] ^= 1;
                    }
                    json!({"schemaVersion":1,"metadata":metadata,"offset":offset,"nextOffset":end,"eof":end==data.len(),"base64":STANDARD.encode(&bytes),"chunkSHA256":format!("{:x}",Sha256::digest(&bytes))})
                };
                let body = serde_json::to_vec(
                    &json!({"jsonrpc":"2.0","id":request["id"],"result":result}),
                )
                .unwrap();
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(head.as_bytes()).await.unwrap();
                socket.write_all(&body).await.unwrap();
            }
        });
        Self {
            client,
            record,
            bytes,
            server,
        }
    }
}
async fn request(socket: &mut tokio::net::TcpStream) -> Value {
    let mut bytes = Vec::new();
    loop {
        let mut data = [0; 1024];
        let n = socket.read(&mut data).await.unwrap();
        assert!(n > 0 && bytes.len() < 8192);
        bytes.extend_from_slice(&data[..n]);
        if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
            let head = std::str::from_utf8(&bytes[..end]).unwrap();
            let length: usize = head
                .lines()
                .find_map(|l| {
                    l.split_once(':')
                        .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                        .map(|(_, v)| v.trim().parse().unwrap())
                })
                .unwrap();
            if bytes.len() >= end + 4 + length {
                return serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
            }
        }
    }
}
#[tokio::test]
async fn streamed_batch_and_archive_verify_bytes_and_rewrite_every_destination() {
    let fixture = Fixture::new(false).await;
    let batch = Batch::new(&fixture.record).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let root =
        scanner_export::native::batch(&fixture.client, &batch, directory.path().to_path_buf())
            .await
            .unwrap();
    for entry in batch.entries() {
        assert_eq!(
            std::fs::read(root.join(&entry.name)).unwrap(),
            fixture.bytes
        );
    }
    assert!(root.join("manifest.json").exists());
    assert!(
        scanner_export::native::batch(&fixture.client, &batch, directory.path().to_path_buf())
            .await
            .is_err()
    );
    let archive = scanner_export::archive(&fixture.client, &batch)
        .await
        .unwrap();
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).unwrap();
    assert_eq!(zip.len(), 3);
    for entry in batch.entries() {
        let mut bytes = Vec::new();
        zip.by_name(&format!("B-001/{}", entry.name))
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes, fixture.bytes);
    }
    let mut json = Vec::new();
    zip.by_name("B-001/manifest.json")
        .unwrap()
        .read_to_end(&mut json)
        .unwrap();
    let manifest: ScanRecord = serde_json::from_slice(&json).unwrap();
    assert_eq!(
        manifest.processed_images.unwrap()[0].image.relative_path,
        batch.entries()[1].name
    );
}
#[tokio::test]
async fn corruption_retains_verified_files_without_a_completion_manifest_or_archive() {
    let fixture = Fixture::new(true).await;
    let batch = Batch::new(&fixture.record).unwrap();
    let directory = tempfile::tempdir().unwrap();
    assert!(
        scanner_export::native::batch(&fixture.client, &batch, directory.path().to_path_buf())
            .await
            .is_err()
    );
    let root = directory.path().join(batch.name());
    assert_eq!(
        std::fs::read(root.join("C-001.png")).unwrap(),
        fixture.bytes
    );
    assert!(!root.join("manifest.json").exists());
    assert!(!root.join(&batch.entries()[1].name).exists());
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 1);
    assert!(
        scanner_export::archive(&fixture.client, &batch)
            .await
            .is_err()
    );
}
