//! MCP 2025-06-18 tools over bounded newline-delimited JSON streams.
mod catalogue;
mod field;
mod framing;
mod protocol;
mod server;
mod tool;
use scanner_types::error::Result;
use serde_json::Value;
pub use server::Server;

/// Authenticated scanner operations supplied by the application. Dropping a
/// request must not cancel physical scans already accepted by the coordinator.
#[async_trait::async_trait]
pub trait Service: Send + Sync + 'static {
    async fn call(&self, method: &str, parameters: Value) -> Result<Value>;
}
#[async_trait::async_trait]
impl Service for scanner_client::Client {
    async fn call(&self, method: &str, parameters: Value) -> Result<Value> {
        self.call(method, parameters).await
    }
}
