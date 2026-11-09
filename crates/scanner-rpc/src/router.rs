use crate::{Failure, Request, Response, params::Params};
use scanner_core::Engine;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub struct Router {
    pub(crate) engine: Arc<Engine>,
    pub(crate) backend_name: String,
}
impl Router {
    pub fn new(engine: Arc<Engine>, backend_name: impl Into<String>) -> Self {
        Self {
            engine,
            backend_name: backend_name.into(),
        }
    }
    /// Version-one requests require an ID; batches/notifications are not part of
    /// the existing scanner protocol. Camera payloads will have a separate bound.
    pub async fn respond(&self, bytes: &[u8]) -> Response {
        if bytes.len() > 64 * 1024 {
            return Response::new(
                Value::Null,
                Err(Failure::new(-32600, "RPC request exceeds 64 KiB.")),
            );
        }
        let parsed: Value = match serde_json::from_slice(bytes) {
            Ok(value) => value,
            Err(_) => {
                return Response::new(Value::Null, Err(Failure::new(-32700, "Invalid JSON.")));
            }
        };
        let request: Request = match serde_json::from_value(parsed) {
            Ok(value) => value,
            Err(_) => {
                return Response::new(
                    Value::Null,
                    Err(Failure::new(-32600, "Invalid JSON-RPC request.")),
                );
            }
        };
        if request.jsonrpc != "2.0" || !matches!(request.id, Value::String(_) | Value::Number(_)) {
            return Response::new(
                request.id,
                Err(Failure::new(
                    -32600,
                    "Expected JSON-RPC 2.0 and a string or numeric ID.",
                )),
            );
        }
        let result = match Params::new(request.params) {
            Ok(params) => self.dispatch(&request.method, params).await,
            Err(error) => Err(error),
        };
        Response::new(request.id, result)
    }
    async fn dispatch(&self, method: &str, params: Params) -> Result<Value, Failure> {
        if let Some(result) = self.scanner(method, &params).await {
            return result;
        }
        if let Some(result) = self.library(method, &params).await {
            return result;
        }
        if let Some(result) = self.profiles(method, &params).await {
            return result;
        }
        Err(Failure::new(-32601, format!("Unknown method: {method}")))
    }
}
