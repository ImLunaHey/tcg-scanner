use scanner_types::error::ScannerError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Failure {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ScannerError>,
}
impl Failure {
    pub(crate) fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
    pub(crate) fn params(message: impl Into<String>) -> Self {
        Self::new(-32602, message)
    }
    pub(crate) fn internal(_: impl std::fmt::Display) -> Self {
        Self::new(-32603, "Internal service error.")
    }
}
impl From<ScannerError> for Failure {
    fn from(error: ScannerError) -> Self {
        Self {
            code: -32000,
            message: error.message.clone(),
            data: Some(error),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Failure>,
}
impl Response {
    pub(crate) fn new(id: Value, result: Result<Value, Failure>) -> Self {
        let (result, error) = match result {
            Ok(value) => (Some(value), None),
            Err(error) => (None, Some(error)),
        };
        Self {
            jsonrpc: "2.0".into(),
            id,
            result,
            error,
        }
    }
}
pub(crate) fn value(value: impl Serialize) -> Result<Value, Failure> {
    serde_json::to_value(value).map_err(Failure::internal)
}
