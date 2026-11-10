use crate::{Failure, Router, model::value, params::Params};
use scanner_types::identification::ReviewRequest;
use serde_json::{Value, json};
impl Router {
    pub(crate) async fn recognition(
        &self,
        method: &str,
        params: &Params,
    ) -> Option<Result<Value, Failure>> {
        if !matches!(
            method,
            "v1.images.identification"
                | "v1.images.review"
                | "v1.images.identify"
                | "v1.images.prepareCard"
        ) {
            return None;
        }
        Some(self.recognition_action(method, params).await)
    }
    async fn recognition_action(&self, method: &str, params: &Params) -> Result<Value, Failure> {
        if method == "v1.images.prepareCard" {
            params.only(&["id", "jobID", "sequence"])?;
            return value(
                self.engine
                    .prepare_card(
                        params.uuid("jobID")?,
                        params.integer("sequence", 0, 1, 1000)? as u32,
                        params.uuid("id")?,
                    )
                    .await?,
            );
        }
        if method == "v1.images.identify" {
            params.only(&["id", "jobID", "sequence", "language"])?;
            let job = params.uuid("jobID")?;
            let language = match params.optional_identifier("language")?.as_deref() {
                Some("en") => scanner_types::cards::CardLanguage::English,
                Some("ko") => scanner_types::cards::CardLanguage::Korean,
                Some(_) => return Err(Failure::params("Language must be en or ko.")),
                None => self
                    .engine
                    .scan_status(job)?
                    .request
                    .card_language
                    .unwrap_or_default(),
            };
            let request = scanner_types::identification::IdentificationRequest {
                id: params.uuid("id")?,
                job_id: job,
                sequence: params.integer("sequence", 0, 1, 1000)? as u32,
                language,
                extensions: Default::default(),
            };
            return value(self.engine.identify_card(request).await?);
        }
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
