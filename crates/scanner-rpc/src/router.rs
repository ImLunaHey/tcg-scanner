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
    /// the existing scanner protocol. Only camera lookup has a larger body bound.
    pub async fn respond(&self, bytes: &[u8]) -> Response {
        if bytes.len() > scanner_types::camera::MAX_REQUEST_BYTES {
            return Response::new(
                Value::Null,
                Err(Failure::new(-32600, "RPC request exceeds 768 KiB.")),
            );
        }
        if bytes.len() > 65536 && !is_camera_request(bytes) {
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
        if request.method != "v1.camera.identify" && bytes.len() > 64 * 1024 {
            return Response::new(
                request.id,
                Err(Failure::new(-32600, "RPC request exceeds 64 KiB.")),
            );
        }
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
        if method == "v1.camera.identify" {
            let result = self.engine.identify_camera(params.decode()?).await?;
            return serde_json::to_value(result)
                .map_err(|_| Failure::new(-32603, "Could not encode camera result."));
        }
        if let Some(result) = self.scanner(method, &params).await {
            return result;
        }
        if let Some(result) = self.library(method, &params).await {
            return result;
        }
        if let Some(result) = self.inventory(method, &params).await {
            return result;
        }
        if let Some(result) = self.profiles(method, &params).await {
            return result;
        }
        if let Some(result) = self.images(method, &params).await {
            return result;
        }
        if let Some(result) = self.recognition(method, &params).await {
            return result;
        }
        if let Some(result) = self.controls(method, &params).await {
            return result;
        }
        Err(Failure::new(-32601, format!("Unknown method: {method}")))
    }
}

/// Inspect only the method for route-specific body limits; this grants no authority.
/// Full envelope/parameter validation still runs before dispatch.
pub fn is_camera_request(bytes: &[u8]) -> bool {
    #[derive(serde::Deserialize)]
    struct Method {
        method: String,
    }
    bytes.len() <= scanner_types::camera::MAX_REQUEST_BYTES
        && serde_json::from_slice::<Method>(bytes).is_ok_and(|v| v.method == "v1.camera.identify")
}
