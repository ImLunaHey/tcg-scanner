use crate::{Failure, Router, model::value, params::Params};
use scanner_types::identification::ReviewRequest;
use serde_json::{Value, json};
impl Router {
    pub(crate) async fn recognition(
        &self,
        method: &str,
        params: &Params,
    ) -> Option<Result<Value, Failure>> {
        if !matches!(method, "v1.images.identification" | "v1.images.review") {
            return None;
        }
        Some(self.recognition_action(method, params).await)
    }
    async fn recognition_action(&self, method: &str, params: &Params) -> Result<Value, Failure> {
        if method == "v1.images.review" {
            params.only(&[
                "jobID",
                "sequence",
                "identificationID",
                "cardID",
                "finish",
                "condition",
            ])?;
            let request: ReviewRequest = params.decode()?;
            return value(self.engine.review_card(request).await?);
        }
        params.only(&["jobID", "sequence"])?;
        let result = self
            .engine
            .card_identification(
                params.uuid("jobID")?,
                params.integer("sequence", 0, 1, 1000)? as u32,
            )
            .await?;
        Ok(json!({"schemaVersion":1,"identification":result}))
    }
}
