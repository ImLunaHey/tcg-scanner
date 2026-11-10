use crate::{Failure, Router, model::value, params::Params};
use serde_json::Value;
impl Router {
    pub(crate) async fn inventory(
        &self,
        method: &str,
        params: &Params,
    ) -> Option<Result<Value, Failure>> {
        if !matches!(
            method,
            "v1.inventory.list" | "v1.inventory.missing" | "v1.inventory.reference"
        ) {
            return None;
        }
        Some(self.inventory_action(method, params).await)
    }
    async fn inventory_action(&self, method: &str, params: &Params) -> Result<Value, Failure> {
        match method {
            "v1.inventory.list" => {
                params.only(&["limit", "offset", "search", "set"])?;
                value(
                    self.engine
                        .inventory(
                            params.integer("limit", 100, 1, 100)? as usize,
                            params.integer("offset", 0, 0, 1_000_000)? as usize,
                            params.filter("search", 160)?,
                            params.filter("set", 512)?,
                        )
                        .await?,
                )
            }
            "v1.inventory.missing" => {
                params.only(&["set"])?;
                value(
                    self.engine
                        .missing_inventory_cards(&params.identifier("set")?)
                        .await?,
                )
            }
            _ => {
                params.only(&["key"])?;
                value(
                    self.engine
                        .inventory_reference(&params.identifier("key")?)
                        .await?,
                )
            }
        }
    }
}
