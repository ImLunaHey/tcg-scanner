use crate::tool::Tool;
use serde_json::{Value, json};
#[derive(Default)]
pub(crate) struct Protocol {
    initialized: bool,
    ready: bool,
}
pub(crate) enum Action {
    Reply(Value),
    Call(Value),
    Cancel(Value),
    Ignore,
}
pub(crate) fn success(id: Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}
pub(crate) fn failure(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
pub(crate) fn valid_id(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| s.len() <= 128 && !s.is_empty())
        || value
            .as_i64()
            .is_some_and(|n| n.unsigned_abs() <= 9_007_199_254_740_991)
}
impl Protocol {
    pub fn receive(&mut self, bytes: &[u8], tools: &[Tool]) -> Action {
        let value: Value = match serde_json::from_slice(bytes) {
            Ok(v) => v,
            Err(_) => return Action::Reply(failure(Value::Null, -32700, "Invalid JSON.")),
        };
        let id = value.get("id").cloned();
        let method = value["method"].as_str();
        if value["jsonrpc"] != "2.0"
            || method.is_none()
            || id.as_ref().is_some_and(|id| !valid_id(id))
            || value.get("params").is_some_and(|v| !v.is_object())
        {
            return Action::Reply(failure(Value::Null, -32600, "Invalid JSON-RPC request."));
        }
        let method = method.unwrap();
        let params = value.get("params").cloned().unwrap_or(json!({}));
        let Some(id) = id else {
            return self.notification(method, params);
        };
        if method == "ping" {
            return Action::Reply(success(id, json!({})));
        }
        if method == "initialize" {
            return self.initialize(id, params);
        }
        if !self.ready {
            return Action::Reply(failure(id, -32000, "Initialize this session first."));
        }
        match method {
            "tools/list" if params.get("cursor").is_none() => Action::Reply(success(
                id,
                json!({"tools":tools.iter().map(Tool::schema).collect::<Vec<_>>()}),
            )),
            "tools/list" => Action::Reply(failure(id, -32602, "Invalid tools cursor.")),
            "tools/call" => Action::Call(value),
            _ => Action::Reply(failure(id, -32601, "Unknown MCP method.")),
        }
    }
    fn notification(&mut self, method: &str, params: Value) -> Action {
        match method {
            "notifications/initialized" if self.initialized => {
                self.ready = true;
                Action::Ignore
            }
            "notifications/cancelled" if self.ready && valid_id(&params["requestId"]) => {
                Action::Cancel(params["requestId"].clone())
            }
            _ => Action::Ignore,
        }
    }
    fn initialize(&mut self, id: Value, params: Value) -> Action {
        if self.initialized {
            return Action::Reply(failure(id, -32600, "Session already initialized."));
        }
        if !params["protocolVersion"].is_string()
            || !params["capabilities"].is_object()
            || !params["clientInfo"]["name"].is_string()
            || !params["clientInfo"]["version"].is_string()
        {
            return Action::Reply(failure(id, -32602, "Missing initialization information."));
        }
        self.initialized = true;
        Action::Reply(success(
            id,
            json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"ScannerMCP","version":env!("CARGO_PKG_VERSION")},"instructions":"Use explicit request IDs for physical scans. Cancelling an MCP request or disconnecting does not cancel an accepted scan; use scanner_cancel_job. Inspect scan state/errors, not only query success."}),
        ))
    }
}
