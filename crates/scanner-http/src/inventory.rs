use crate::{
    Gateway,
    error::{Error, Result},
    images::cached,
    path::Path,
};
use axum::{
    Json,
    body::Body,
    http::request::Parts,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::json;
use sha2::{Digest, Sha256};
impl Gateway {
    pub(crate) async fn inventory_route(
        &self,
        head: &Parts,
        path: &Path,
        parts: &[&str],
    ) -> Result<Response> {
        if head.method != "GET" {
            return Err(Error::new(
                405,
                "methodNotAllowed",
                "Use GET for inventory.",
            ));
        }
        match parts {
            ["inventory"] => {
                path.only(&["limit", "offset", "search", "set"])?;
                let value=self.call("inventory.list",json!({
                    "limit":path.number("limit",100,1,100)?,"offset":path.number("offset",0,0,1_000_000)?,
                    "search":path.query.get("search").map(String::as_str).unwrap_or(""),
                    "set":path.query.get("set").map(String::as_str).unwrap_or("")
                })).await?;
                Ok(Json(value).into_response())
            }
            ["inventory", "missing"] => {
                path.only(&["set"])?;
                let set = path
                    .query
                    .get("set")
                    .ok_or_else(|| Error::params("Provide an inventory set."))?;
                Ok(Json(self.call("inventory.missing", json!({"set":set})).await?).into_response())
            }
            ["inventory", "references", key, "thumbnail"] => {
                path.only(&[])?;
                let result = self.engine.inventory_reference(key).await?;
                let bytes = STANDARD
                    .decode(result.base64)
                    .map_err(|_| Error::internal())?;
                if format!("{:x}", Sha256::digest(&bytes)) != result.sha256 {
                    return Err(Error::internal());
                }
                cached(head, Body::from(bytes), &result.media_type, &result.sha256)
            }
            _ => Err(Error::new(404, "notFound", "Unknown inventory route.")),
        }
    }
}
