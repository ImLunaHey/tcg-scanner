use crate::{Failure, Router, model::value, params::Params};
use scanner_types::scan::ScanRequest;
use serde_json::{Value, json};

impl Router {
    pub(crate) async fn scanner(
        &self,
        method: &str,
        params: &Params,
    ) -> Option<Result<Value, Failure>> {
        if !matches!(
            method,
            "v1.devices"
                | "v1.capabilities"
                | "v1.settings"
                | "v1.diagnostics"
                | "v1.scan.start"
                | "v1.jobs.create"
                | "v1.scan.status"
                | "v1.scan.cancel"
        ) {
            return None;
        }
        Some(self.scanner_action(method, params).await)
    }
    async fn scanner_action(&self, method: &str, params: &Params) -> Result<Value, Failure> {
        match method {
            "v1.scan.start" | "v1.jobs.create" => {
                params.only(&[
                    "requestID",
                    "deviceID",
                    "outputDirectory",
                    "dpi",
                    "duplex",
                    "format",
                    "profileID",
                    "useDeviceSettings",
                    "prepareCards",
                    "background",
                    "overscan",
                    "cardLanguage",
                ])?;
                let request: ScanRequest = params.decode()?;
                value(self.engine.start_scan(request).await?)
            }
            "v1.scan.status" | "v1.scan.cancel" => {
                params.only(&["scanID"])?;
                let id = params.uuid("scanID")?;
                if method == "v1.scan.cancel" {
                    value(self.engine.cancel_scan(id).await?)
                } else {
                    self.record(id).await
                }
            }
            "v1.devices" => {
                params.only(&[])?;
                Ok(json!({"schemaVersion":1,"devices":self.engine.devices().await?}))
            }
            _ => {
                params.only(&["deviceID"])?;
                let id = params.optional_identifier("deviceID")?;
                if method == "v1.diagnostics" {
                    return self.diagnostics(id).await;
                }
                let mut report = self.engine.inspect(id).await?;
                if method == "v1.capabilities" {
                    return value(report);
                }
                for unit in &mut report.units {
                    for setting in unit.settings.values_mut() {
                        setting.allowed = None;
                        setting.default_value = None;
                    }
                    for feature in &mut unit.vendor_features {
                        if let Some(fields) =
                            feature.get_mut("fields").and_then(Value::as_object_mut)
                        {
                            fields.retain(|key, _| key == "current");
                        }
                    }
                }
                let device_id = report.device.id.clone();
                let engine = self.engine.clone();
                let configured = tokio::task::spawn_blocking(move || {
                    engine.database().device_settings(&device_id)
                })
                .await
                .map_err(Failure::internal)??;
                let mut result = json!({"schemaVersion":1,"device":report.device,"observedAt":report.observed_at,"units":report.units,"warnings":report.warnings,"errors":report.errors});
                if let Some(settings) = configured {
                    result["configuredSettings"] = settings;
                }
                Ok(result)
            }
        }
    }
    async fn diagnostics(&self, id: Option<String>) -> Result<Value, Failure> {
        let devices = self.engine.devices().await?;
        let inspection = if id.is_some() || devices.len() == 1 {
            Some(self.engine.inspect(id.clone()).await?)
        } else {
            None
        };
        let warnings: Vec<&str> = if devices.is_empty() {
            vec!["No scanner found. Check its connection, power and installed driver."]
        } else if devices.len() > 1 && id.is_none() {
            vec!["Specify a device ID to inspect one of the connected scanners."]
        } else {
            vec![]
        };
        let mut result = json!({"schemaVersion":1,"operatingSystem":std::env::consts::OS,"backend":self.backend_name,"devices":devices,"warnings":warnings});
        if let Some(inspection) = inspection {
            result["inspection"] = value(inspection)?;
        }
        Ok(result)
    }
}
