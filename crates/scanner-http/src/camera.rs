use crate::{
    Gateway,
    error::{Error, Result},
    path::Path,
};
use axum::{
    Json,
    http::request::Parts,
    response::{IntoResponse, Response},
};
use serde_json::{Map, Value};
use std::sync::Arc;
pub(crate) fn is_route(path: &Path) -> bool {
    path.parts == ["api", "v1", "camera", "identify"]
        || path.parts == ["api", "v1", "camera", "price"]
}
impl Gateway {
    pub(crate) async fn camera_route(
        self: &Arc<Self>,
        head: &Parts,
        path: &Path,
        body: Map<String, Value>,
    ) -> Result<Response> {
        path.only(&[])?;
        if head.method != "POST" {
            return Err(Error::new(
                405,
                "methodNotAllowed",
                "Use POST for camera lookup.",
            ));
        }
        if path.parts.last().is_some_and(|p| p == "import") {
            let capture = serde_json::from_value(Value::Object(body)).map_err(|_| {
                Error::params("Provide a capture ID, selected card, JPEG and quantity.")
            })?;
            return Ok(Json(self.engine.import_camera(capture).await?).into_response());
        }
        if path.parts.last().is_some_and(|p| p == "price") {
            let selection = serde_json::from_value(Value::Object(body)).map_err(|_| {
                Error::params("Provide a selected card, game, language and finish.")
            })?;
            return Ok(Json(self.engine.camera_price(selection).await?).into_response());
        }
        let request = serde_json::from_value(Value::Object(body))
            .map_err(|_| Error::params("Provide base64 JPEG and language en or ko."))?;
        Ok(Json(self.engine.identify_camera(request).await?).into_response())
    }
}
