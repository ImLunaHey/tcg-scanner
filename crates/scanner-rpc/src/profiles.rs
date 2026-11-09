use crate::{Failure, Router, model::value, params::Params};
use scanner_types::service::{ProfilePatch, ScanProfile};
use serde_json::{Value, json};

impl Router {
    pub(crate) async fn profiles(
        &self,
        method: &str,
        params: &Params,
    ) -> Option<Result<Value, Failure>> {
        if !matches!(
            method,
            "v1.profiles.list"
                | "v1.profiles.get"
                | "v1.profiles.save"
                | "v1.profiles.create"
                | "v1.profiles.update"
                | "v1.profiles.delete"
        ) {
            return None;
        }
        Some(self.profile_action(method, params).await)
    }
    async fn profile_action(&self, method: &str, params: &Params) -> Result<Value, Failure> {
        match method {
            "v1.profiles.list" => {
                params.only(&[])?;
                Ok(json!({"schemaVersion":1,"profiles":self.engine.profiles().await?}))
            }
            "v1.profiles.get" => {
                params.only(&["id"])?;
                value(self.engine.profile(params.identifier("id")?).await?)
            }
            "v1.profiles.delete" => {
                params.only(&["id"])?;
                self.engine.remove_profile(params.identifier("id")?).await?;
                Ok(json!({"deleted":true}))
            }
            _ => {
                params.only(&["id", "name", "dpi", "duplex", "format"])?;
                if method == "v1.profiles.update" {
                    if params.0.values().any(Value::is_null) {
                        return Err(Failure::params(
                            "Profile updates cannot contain null fields.",
                        ));
                    }
                    let id = params.identifier("id")?;
                    let mut fields = params.0.clone();
                    fields.remove("id");
                    let patch: ProfilePatch = Params(fields).decode()?;
                    return value(self.engine.update_profile(id, patch).await?);
                }
                let profile: ScanProfile = params.decode()?;
                if method == "v1.profiles.create" {
                    value(self.engine.create_profile(profile).await?)
                } else {
                    value(self.engine.save_profile(profile).await?)
                }
            }
        }
    }
}
