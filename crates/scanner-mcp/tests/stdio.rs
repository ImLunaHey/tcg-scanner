use scanner_mcp::{Server, Service};
use scanner_types::error::{Result, ScannerError};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream},
    sync::Notify,
    task::JoinHandle,
};
#[derive(Default)]
struct Api {
    calls: Mutex<Vec<(String, Value)>>,
    release: Notify,
}
#[async_trait::async_trait]
impl Service for Api {
    async fn call(&self, method: &str, params: Value) -> Result<Value> {
        self.calls
            .lock()
            .unwrap()
            .push((method.into(), params.clone()));
        if method == "events" {
            self.release.notified().await;
        }
        if method == "jobs.get" {
            return Err(ScannerError::new("jobNotFound", "No saved job."));
        }
        Ok(json!({"method":method,"parameters":params}))
    }
}
struct Session {
    input: DuplexStream,
    output: BufReader<DuplexStream>,
    task: JoinHandle<std::io::Result<()>>,
    api: Arc<Api>,
}
impl Session {
    fn new(read_only: bool) -> Self {
        let (input, read) = tokio::io::duplex(131072);
        let (write, output) = tokio::io::duplex(131072);
        let api = Arc::new(Api::default());
        let server = Server::new(api.clone(), read_only);
        let task = tokio::spawn(async move { server.serve(read, write).await });
        Self {
            input,
            output: BufReader::new(output),
            task,
            api,
        }
    }
    async fn send(&mut self, value: Value) {
        let mut bytes = value.to_string().into_bytes();
        bytes.push(b'\n');
        self.input.write_all(&bytes).await.unwrap();
    }
    async fn next(&mut self) -> Value {
        let mut line = String::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            self.output.read_line(&mut line),
        )
        .await
        .unwrap()
        .unwrap();
        serde_json::from_str(&line).unwrap()
    }
    async fn initialize(&mut self) {
        self.send(json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"future","capabilities":{},"clientInfo":{"name":"test","version":"1"}}})).await;
        let value = self.next().await;
        assert_eq!(value["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(
            value["result"]["capabilities"],
            json!({"tools":{"listChanged":false}})
        );
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
    }
    async fn call(&mut self, id: u32, name: &str, args: Value) {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).await;
    }
    async fn close(mut self) {
        self.input.shutdown().await.unwrap();
        self.task.await.unwrap().unwrap();
    }
}
#[tokio::test]
async fn initialization_read_only_and_schemas_gate_dispatch() {
    let mut s = Session::new(true);
    s.call(1, "scanner_list_devices", json!({})).await;
    assert_eq!(s.next().await["error"]["code"], -32000);
    s.initialize().await;
    s.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
        .await;
    let value = s.next().await;
    let tools = value["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 18);
    assert!(
        tools
            .iter()
            .all(|t| t["annotations"]["readOnlyHint"] == true)
    );
    s.call(
        3,
        "scanner_start_scan",
        json!({"requestID":uuid::Uuid::new_v4()}),
    )
    .await;
    assert_eq!(s.next().await["error"]["code"], -32602);
    assert!(s.api.calls.lock().unwrap().is_empty());
    s.call(4, "scanner_get_service_status", json!({})).await;
    let result = s.next().await["result"].clone();
    assert_eq!(result["isError"], false);
    let text: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(result["structuredContent"], text);
    assert_eq!(text["data"]["method"], "service.status");
    s.close().await;
}
#[tokio::test]
async fn arguments_defaults_and_domain_errors_preserve_tool_contract() {
    let mut s = Session::new(false);
    s.initialize().await;
    for (id, args) in [
        (1, json!({})),
        (2, json!({"requestID":"bad"})),
        (3, json!({"requestID":uuid::Uuid::new_v4(),"dpi":601})),
        (
            4,
            json!({"requestID":uuid::Uuid::new_v4(),"unsafeOverride":true}),
        ),
    ] {
        s.call(id, "scanner_start_scan", args).await;
        let result = s.next().await;
        assert_eq!(result["result"]["isError"], true);
        assert_eq!(
            result["result"]["structuredContent"]["error"]["kind"],
            "invalidParams"
        );
    }
    assert!(s.api.calls.lock().unwrap().is_empty());
    let request_id = uuid::Uuid::new_v4();
    s.call(5, "scanner_start_scan", json!({"requestID":request_id}))
        .await;
    let result = s.next().await;
    assert_eq!(
        result["result"]["structuredContent"]["data"]["parameters"],
        json!({"requestID":request_id,"dpi":600,"duplex":false,"format":"png"})
    );
    s.call(6, "scanner_get_job", json!({"id":request_id})).await;
    let result = s.next().await;
    assert_eq!(result["result"]["isError"], true);
    assert_eq!(
        result["result"]["structuredContent"]["error"]["kind"],
        "jobNotFound"
    );
    s.close().await;
}
#[tokio::test]
async fn concurrent_calls_are_bounded_and_mcp_cancellation_never_cancels_scan() {
    let mut s = Session::new(false);
    s.initialize().await;
    for id in 1..=9 {
        s.call(id, "scanner_get_events", json!({"waitSeconds":20}))
            .await;
    }
    let busy = s.next().await;
    assert_eq!(busy["id"], 9);
    assert_eq!(busy["error"]["code"], -32000);
    s.call(1, "scanner_get_events", json!({})).await;
    assert_eq!(s.next().await["error"]["code"], -32600);
    s.send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}))
        .await;
    s.send(json!({"jsonrpc":"2.0","id":10,"method":"ping"}))
        .await;
    assert_eq!(s.next().await["id"], 10);
    assert!(
        s.api
            .calls
            .lock()
            .unwrap()
            .iter()
            .all(|(method, _)| method == "events")
    );
    s.close().await;
}
#[tokio::test]
async fn oversized_or_truncated_frames_close_without_dispatch() {
    for bytes in [vec![b'x'; 65537], b"{\"jsonrpc\":\"2.0\"".to_vec()] {
        let mut s = Session::new(false);
        s.input.write_all(&bytes).await.unwrap();
        s.input.shutdown().await.unwrap();
        assert!(s.task.await.unwrap().is_err());
        assert!(s.api.calls.lock().unwrap().is_empty());
    }
}
