use crate::{
    Gateway,
    error::{Error, Result},
    path::Path,
    routes::only,
};
use axum::{
    Json,
    http::request::Parts,
    response::{IntoResponse, Response},
};
use serde_json::{Map, Value, json};
use uuid::Uuid;
impl Gateway {
    pub(crate) async fn finish_route(
        &self,
        path: &Path,
        job: Uuid,
        sequence: u32,
        mut body: Map<String, Value>,
    ) -> Result<Response> {
        path.only(&[])?;
        only(&body, &["identificationID"])?;
        body.insert("jobID".into(), json!(job));
        body.insert("sequence".into(), json!(sequence));
        Ok(Json(self.call("images.estimateFinish", json!(body)).await?).into_response())
    }
    pub(crate) async fn condition_route(
        &self,
        path: &Path,
        job: Uuid,
        sequence: u32,
        mut body: Map<String, Value>,
    ) -> Result<Response> {
        path.only(&[])?;
        only(&body, &["identificationID"])?;
        body.insert("jobID".into(), json!(job));
        body.insert("sequence".into(), json!(sequence));
        Ok(Json(self.call("images.checkCondition", json!(body)).await?).into_response())
    }
    pub(crate) async fn prepare_card_route(
        &self,
        path: &Path,
        job: Uuid,
        sequence: u32,
        mut body: Map<String, Value>,
    ) -> Result<Response> {
        path.only(&[])?;
        only(&body, &["id"])?;
        body.insert("jobID".into(), json!(job));
        body.insert("sequence".into(), json!(sequence));
        Ok((
            axum::http::StatusCode::CREATED,
            Json(self.call("images.prepareCard", json!(body)).await?),
        )
            .into_response())
    }

    pub(crate) async fn identification_route(
        &self,
        head: &Parts,
        path: &Path,
        job: Uuid,
        sequence: u32,
        mut body: Map<String, Value>,
    ) -> Result<Response> {
        path.only(&[])?;
        let method = match head.method.as_str() {
            "GET" => {
                only(&body, &[])?;
                "images.identification"
            }
            "POST" => {
                only(&body, &["id", "language"])?;
                "images.identify"
            }
            "PATCH" => {
                only(
                    &body,
                    &["identificationID", "cardID", "finish", "condition"],
                )?;
                "images.review"
            }
            _ => {
                return Err(Error::new(
                    405,
                    "methodNotAllowed",
                    "Use GET, POST or PATCH for identification.",
                ));
            }
        };
        body.insert("jobID".into(), json!(job));
        body.insert("sequence".into(), json!(sequence));
        Ok((
            if head.method == "POST" {
                axum::http::StatusCode::ACCEPTED
            } else {
                axum::http::StatusCode::OK
            },
            Json(self.call(method, json!(body)).await?),
        )
            .into_response())
    }
}
