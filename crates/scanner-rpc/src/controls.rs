use crate::{Failure, Router, model::value, params::Params};
use scanner_types::service::ScanSettings;
use serde_json::Value;

impl Router {
    pub(crate) async fn controls(
        &self,
        method: &str,
        params: &Params,
    ) -> Option<Result<Value, Failure>> {
        if !matches!(method, "v1.device.get" | "v1.settings.set") {
            return None;
        }
        Some(self.control_action(method, params).await)
    }
    async fn control_action(&self, method: &str, params: &Params) -> Result<Value, Failure> {
        if method == "v1.device.get" {
            params.only(&["deviceID"])?;
            return value(
                self.engine
                    .device_status(params.optional_identifier("deviceID")?)
                    .await?,
            );
        }
        params.only(&["deviceID", "dpi", "duplex", "format"])?;
        let id = params.optional_identifier("deviceID")?;
        let mut fields = params.0.clone();
        fields.remove("deviceID");
        let settings: ScanSettings = Params(fields).decode()?;
        value(self.engine.set_settings(id, settings).await?)
    }
}
