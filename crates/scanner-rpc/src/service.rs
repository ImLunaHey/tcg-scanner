use crate::{Failure, Router, model::value, params::Params};
use serde_json::Value;
impl Router {
    pub(crate) async fn service(
        &self,
        method: &str,
        params: &Params,
    ) -> Option<Result<Value, Failure>> {
        if !matches!(
            method,
            "v1.service.status"
                | "v1.service.pause"
                | "v1.service.resume"
                | "v1.storage.status"
                | "v1.storage.reconnect"
                | "v1.diagnostics.buttonMonitor"
                | "v1.diagnostics.stopButtonMonitor"
        ) {
            return None;
        }
        Some(self.service_action(method, params).await)
    }
    async fn service_action(&self, method: &str, params: &Params) -> Result<Value, Failure> {
        if method == "v1.diagnostics.buttonMonitor" {
            params.only(&["seconds"])?;
            let seconds = params.integer("seconds", 300, 30, 300)? as u32;
            self.engine.start_button_observation(seconds).await?;
            return Ok(serde_json::json!({"schemaVersion":1,"state":"opening","seconds":seconds}));
        }
        params.only(&[])?;
        match method {
            "v1.diagnostics.stopButtonMonitor" => {
                self.engine.stop_button_observation().await?;
                Ok(serde_json::json!({"schemaVersion":1,"state":"stopped"}))
            }
            "v1.service.status" => value(self.engine.status().await),
            "v1.service.pause" => value(self.engine.pause().await?),
            "v1.service.resume" => value(self.engine.resume().await?),
            "v1.storage.status" => value(self.engine.storage_status().await?),
            _ => value(self.engine.reconnect_storage().await?),
        }
    }
}
