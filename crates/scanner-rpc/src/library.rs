use crate::{Failure, Router, model::value, params::Params};
use serde_json::Value;

impl Router {
    pub(crate) async fn library(
        &self,
        method: &str,
        params: &Params,
    ) -> Option<Result<Value, Failure>> {
        if !matches!(
            method,
            "v1.batches.continue"
                | "v1.jobs.list"
                | "v1.batches.list"
                | "v1.jobs.get"
                | "v1.batches.get"
                | "v1.batches.valuation"
                | "v1.jobs.cancel"
                | "v1.service.status"
                | "v1.events"
        ) {
            return None;
        }
        Some(self.library_action(method, params).await)
    }
    async fn library_action(&self, method: &str, params: &Params) -> Result<Value, Failure> {
        match method {
            "v1.batches.valuation" => {
                params.only(&["id"])?;
                value(self.engine.batch_valuation(params.uuid("id")?).await?)
            }
            "v1.service.status" => {
                params.only(&[])?;
                value(self.engine.status().await)
            }
            "v1.events" => {
                params.only(&["after", "limit", "waitSeconds"])?;
                value(
                    self.engine
                        .events(
                            params.integer("after", 0, 0, 9_007_199_254_740_991)? as i64,
                            params.integer("limit", 100, 1, 100)? as usize,
                            params.integer("waitSeconds", 0, 0, 20)? as u32,
                        )
                        .await?,
                )
            }
            "v1.jobs.list" | "v1.batches.list" => {
                params.only(&["limit", "offset"])?;
                value(
                    self.engine
                        .jobs(
                            params.integer("limit", 50, 1, 100)? as usize,
                            params.integer("offset", 0, 0, 1_000_000)? as usize,
                        )
                        .await?,
                )
            }
            "v1.batches.continue" => {
                params.only(&["id", "requestID"])?;
                value(
                    self.engine
                        .continue_batch(params.uuid("id")?, params.uuid("requestID")?)
                        .await?,
                )
            }
            "v1.jobs.cancel" => {
                params.only(&["id"])?;
                value(self.engine.cancel_scan(params.uuid("id")?).await?)
            }
            _ => {
                params.only(&["id"])?;
                self.record(params.uuid("id")?).await
            }
        }
    }
    pub(crate) async fn record(&self, id: uuid::Uuid) -> Result<Value, Failure> {
        let engine = self.engine.clone();
        value(
            tokio::task::spawn_blocking(move || engine.scan_status(id))
                .await
                .map_err(Failure::internal)??,
        )
    }
}
