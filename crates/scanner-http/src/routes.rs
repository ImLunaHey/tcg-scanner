use crate::{
    Gateway,
    access::Principal,
    error::{Error, Result},
    gateway::header_value,
    path::Path,
};
use axum::{
    Json,
    http::{Method, StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use serde_json::{Map, Value, json};
use std::sync::Arc;
use uuid::Uuid;

pub(crate) fn binary_image(method: &Method, path: &Path) -> bool {
    if method != "GET" {
        return false;
    }
    let p: Vec<_> = path.parts.iter().map(String::as_str).collect();
    matches!(
        p.as_slice(),
        ["api", "v1", "images", _]
            | ["api", "v1", "images", _, "thumbnail"]
            | ["api", "v1", "inventory", "references", _, "thumbnail"]
    )
}
impl Gateway {
    pub(crate) async fn route(
        self: &Arc<Self>,
        head: &Parts,
        path: &Path,
        body: Map<String, Value>,
        principal: &Principal,
    ) -> Result<Response> {
        let p: Vec<_> = path.parts.iter().map(String::as_str).collect();
        let ["api", "v1", rest @ ..] = p.as_slice() else {
            return Err(Error::new(404, "notFound", "Unknown API route."));
        };
        if rest == ["camera", "identify"] {
            return self.camera_route(head, path, body).await;
        }
        if rest == ["session"] {
            path.only(&[])?;
            only(&body, &[])?;
            let mut access = self.access.lock().map_err(|_| Error::internal())?;
            return match head.method.as_str() {
                "POST" => {
                    let (token, expires) = access.issue(principal)?;
                    Ok((
                        StatusCode::CREATED,
                        Json(json!({"schemaVersion":1,"token":token,"expiresAt":expires})),
                    )
                        .into_response())
                }
                "DELETE" => {
                    access.revoke(principal)?;
                    Ok(Json(json!({"revoked":true})).into_response())
                }
                _ => Err(Error::new(
                    405,
                    "methodNotAllowed",
                    "Use POST or DELETE for sessions.",
                )),
            };
        }
        if rest == ["events"] && head.method == "GET" {
            return self.event_stream(head, path, principal.clone()).await;
        }
        if rest.first() == Some(&"inventory") {
            return self.inventory_route(head, path, rest).await;
        }
        if rest.first() == Some(&"images") {
            return self.image_route(head, path, rest, body).await;
        }
        let (method, params, status) = if head.method == "GET" {
            read_route(rest, path)?
        } else {
            write_route(head, rest, path, body)?
        };
        let result = self.call(method, Value::Object(params)).await?;
        let mut response = (StatusCode::from_u16(status).unwrap(), Json(result)).into_response();
        if let ["batches", id, "manifest"] = rest {
            response.headers_mut().insert(
                "content-disposition",
                format!("attachment; filename=\"batch-{}.json\"", uuid(id)?)
                    .parse()
                    .map_err(|_| Error::internal())?,
            );
        }
        Ok(response)
    }
}
fn read_route<'a>(parts: &[&'a str], path: &Path) -> Result<(&'a str, Map<String, Value>, u16)> {
    let mut p = Map::new();
    let method = match parts {
        ["devices"] => "devices",
        ["profiles"] => "profiles.list",
        ["status"] => "service.status",
        ["storage"] => "storage.status",
        ["diagnostics"] => {
            path.only(&["deviceID"])?;
            if let Some(id) = path.query.get("deviceID") {
                p.insert("deviceID".into(), json!(id));
            }
            return Ok(("diagnostics", p, 200));
        }
        ["jobs"] | ["batches"] => {
            path.only(&["limit", "offset"])?;
            p.insert("limit".into(), json!(path.number("limit", 50, 1, 100)?));
            p.insert(
                "offset".into(),
                json!(path.number("offset", 0, 0, 1_000_000)?),
            );
            return Ok((
                if parts[0] == "jobs" {
                    "jobs.list"
                } else {
                    "batches.list"
                },
                p,
                200,
            ));
        }
        ["devices", id] => {
            p.insert("deviceID".into(), json!(id));
            "device.get"
        }
        ["devices", id, kind @ ("settings" | "capabilities")] => {
            p.insert("deviceID".into(), json!(id));
            kind
        }
        [kind @ ("profiles" | "jobs" | "batches"), id] => {
            p.insert("id".into(), json!(uuid(id)?));
            match *kind {
                "profiles" => "profiles.get",
                "jobs" => "jobs.get",
                _ => "batches.get",
            }
        }
        ["batches", id, "store"] => {
            p.insert("id".into(), json!(uuid(id)?));
            "batches.storeStatus"
        }
        ["batches", id, "valuation"] => {
            p.insert("id".into(), json!(uuid(id)?));
            "batches.valuation"
        }
        ["batches", id, "manifest"] => {
            p.insert("id".into(), json!(uuid(id)?));
            "batches.get"
        }
        _ => return Err(Error::new(404, "notFound", "Unknown API route.")),
    };
    path.only(&[])?;
    Ok((method, p, 200))
}
fn write_route(
    head: &Parts,
    parts: &[&str],
    path: &Path,
    mut p: Map<String, Value>,
) -> Result<(&'static str, Map<String, Value>, u16)> {
    path.only(&[])?;
    let (method, status) = match (head.method.as_str(), parts) {
        ("POST", ["service", action @ ("pause" | "resume")]) => {
            only(&p, &[])?;
            (
                if *action == "pause" {
                    "service.pause"
                } else {
                    "service.resume"
                },
                200,
            )
        }
        ("POST", ["storage", "reconnect"]) => {
            only(&p, &[])?;
            ("storage.reconnect", 200)
        }
        ("POST", ["jobs"]) => {
            prepare_job(head, &mut p)?;
            ("jobs.create", 202)
        }
        ("POST", ["profiles"]) => {
            only(&p, &["id", "name", "dpi", "duplex", "format"])?;
            defaults(&mut p);
            ("profiles.create", 201)
        }
        ("PATCH", ["profiles", id]) => {
            only(&p, &["name", "dpi", "duplex", "format"])?;
            p.insert("id".into(), json!(uuid(id)?));
            ("profiles.update", 200)
        }
        ("DELETE", ["profiles", id]) => {
            only(&p, &[])?;
            p.insert("id".into(), json!(uuid(id)?));
            ("profiles.delete", 200)
        }
        ("POST", ["jobs", id, "cancel"]) => {
            only(&p, &[])?;
            p.insert("id".into(), json!(uuid(id)?));
            ("jobs.cancel", 200)
        }
        ("POST", ["batches", id, "store"]) => {
            only(&p, &[])?;
            p.insert("id".into(), json!(uuid(id)?));
            ("batches.sendToStore", 202)
        }
        ("POST", ["batches", id, "continue"]) => {
            only(&p, &["requestID"])?;
            p.insert("id".into(), json!(uuid(id)?));
            ("batches.continue", 202)
        }
        ("PATCH", ["devices", id, "settings"]) => {
            only(&p, &["dpi", "duplex", "format"])?;
            p.insert("deviceID".into(), json!(id));
            ("settings.set", 200)
        }
        _ => {
            return Err(Error::new(
                405,
                "methodNotAllowed",
                "Method or route not supported.",
            ));
        }
    };
    Ok((method, p, status))
}
fn defaults(p: &mut Map<String, Value>) {
    for (key, value) in [
        ("dpi", json!(600)),
        ("duplex", json!(false)),
        ("format", json!("png")),
    ] {
        p.entry(key).or_insert(value);
    }
}
pub(crate) fn only(params: &Map<String, Value>, keys: &[&str]) -> Result<()> {
    if params.keys().any(|k| !keys.contains(&k.as_str())) {
        Err(Error::params("Unknown JSON field."))
    } else {
        Ok(())
    }
}
pub(crate) fn uuid(text: &str) -> Result<String> {
    Uuid::parse_str(text)
        .map(|v| v.to_string())
        .map_err(|_| Error::params("Expected UUID."))
}

fn prepare_job(head: &Parts, p: &mut Map<String, Value>) -> Result<()> {
    only(
        p,
        &[
            "requestID",
            "deviceID",
            "profileID",
            "useDeviceSettings",
            "dpi",
            "duplex",
            "format",
            "prepareCards",
            "background",
            "overscan",
            "cardLanguage",
        ],
    )?;
    if let Some(key) = header_value(head, "idempotency-key".parse().unwrap())? {
        let key = uuid(key)?;
        if let Some(supplied) = p.get("requestID")
            && supplied.as_str().map(uuid).transpose()?.as_deref() != Some(&key)
        {
            return Err(Error::params("Idempotency-Key and requestID must match."));
        }
        p.insert("requestID".into(), json!(key));
    }
    let id = uuid(
        p.get("requestID")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::params("Provide requestID or Idempotency-Key."))?,
    )?;
    p.insert("requestID".into(), json!(id));
    defaults(p);
    Ok(())
}
