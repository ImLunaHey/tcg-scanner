use crate::{
    Configuration,
    access::{Access, Principal},
    error::{Error, Result},
    lease::LeaseBody,
    path::Path,
};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{header, request::Parts},
    response::{IntoResponse, Response},
};
use scanner_core::Engine;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tokio::sync::Semaphore;

pub struct Gateway {
    pub(crate) engine: Arc<Engine>,
    pub(crate) rpc: scanner_rpc::Router,
    configuration: Configuration,
    pub(crate) access: Mutex<Access>,
    web: OnceLock<crate::web::WebAssets>,
    web_downloads: Arc<Semaphore>,
    active: Arc<Semaphore>,
    pub(crate) downloads: Arc<Semaphore>,
    pub(crate) streams: Arc<Semaphore>,
    cameras: Arc<Semaphore>,
}
impl Gateway {
    pub fn new(
        engine: Arc<Engine>,
        backend_name: impl Into<String>,
        token: &str,
        configuration: Configuration,
    ) -> scanner_types::error::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            rpc: scanner_rpc::Router::new(engine.clone(), backend_name),
            engine,
            configuration,
            access: Mutex::new(Access::new(token)?),
            active: Arc::new(Semaphore::new(16)),
            web: OnceLock::new(),
            web_downloads: Arc::new(Semaphore::new(8)),
            downloads: Arc::new(Semaphore::new(2)),
            streams: Arc::new(Semaphore::new(4)),
            cameras: Arc::new(Semaphore::new(1)),
        }))
    }
    /// Configure an immutable, verified UI bundle before accepting connections.
    pub fn configure_web_assets(
        &self,
        assets: crate::web::WebAssets,
    ) -> scanner_types::error::Result<()> {
        self.web.set(assets).map_err(|_| {
            scanner_types::error::ScannerError::new(
                "configuration",
                "Browser assets are already configured.",
            )
        })
    }
    pub fn router(self: &Arc<Self>) -> Router {
        Router::new().fallback(handle).with_state(self.clone())
    }
    pub fn address(&self) -> std::net::SocketAddr {
        self.configuration.address()
    }
    async fn respond(self: &Arc<Self>, request: Request) -> Result<Response> {
        let (head, body) = request.into_parts();
        self.headers(&head)?;
        let path = Path::parse(head.uri.path_and_query().map(|p| p.as_str()).unwrap_or("/"))?;
        let public_image = crate::routes::binary_image(&head.method, &path);
        let public_web = matches!(head.method.as_str(), "GET" | "HEAD")
            && self.web.get().is_some_and(|w| w.contains(&path));
        let camera = head.method == "POST" && crate::camera::is_route(&path);
        let principal = if camera {
            let mut access = self.access.lock().map_err(|_| Error::internal())?;
            if path.parts.last().is_some_and(|p| p == "price") {
                access.price()?
            } else {
                access.camera()?
            }
        } else {
            self.principal(&head, public_image, public_web)?
        };
        let permits = self.request_permits(&path, public_image, public_web)?;
        let rpc = path.parts == ["rpc"];
        let limit = if camera || rpc || path.parts == ["api", "v1", "camera", "import"] {
            scanner_types::camera::MAX_REQUEST_BYTES
        } else {
            60 * 1024
        };
        let body = collect_body(&head, body, limit).await?;
        if rpc && body.len() > 65536 && !scanner_rpc::is_camera_request(&body) {
            return Err(Error::new(
                413,
                "resourceLimit",
                "RPC request exceeds 64 KiB.",
            ));
        }
        if !self
            .access
            .lock()
            .map_err(|_| Error::internal())?
            .valid(&principal)
        {
            return Err(Error::new(
                401,
                "unauthorized",
                "Session expired or revoked.",
            ));
        }
        let response = if public_web {
            if !body.is_empty() {
                return Err(Error::params("Asset requests do not accept a body."));
            }
            self.web
                .get()
                .ok_or_else(Error::internal)?
                .response(&head, &path)?
        } else if rpc {
            path.only(&[])?;
            if head.method != "POST" {
                return Err(Error::new(
                    405,
                    "methodNotAllowed",
                    "Use POST for JSON-RPC.",
                ));
            }
            Json(self.rpc.respond(&body).await).into_response()
        } else {
            let params = if body.is_empty() {
                json!({})
            } else {
                serde_json::from_slice(&body)
                    .map_err(|_| Error::params("Expected a JSON object."))?
            };
            let params = params
                .as_object()
                .cloned()
                .ok_or_else(|| Error::params("Expected a JSON object."))?;
            self.route(&head, &path, params, &principal).await?
        };
        let (head, body) = response.into_parts();
        Ok(Response::from_parts(head, LeaseBody::wrap(body, permits)))
    }
    fn request_permits(
        &self,
        path: &Path,
        public_image: bool,
        public_web: bool,
    ) -> Result<Vec<tokio::sync::OwnedSemaphorePermit>> {
        let mut permits = vec![
            self.active
                .clone()
                .try_acquire_owned()
                .map_err(|_| Error::busy())?,
        ];
        if public_image || public_web {
            permits.push(
                (if public_web {
                    &self.web_downloads
                } else {
                    &self.downloads
                })
                .clone()
                .try_acquire_owned()
                .map_err(|_| Error::busy())?,
            );
        }
        if path.parts == ["api", "v1", "events"] {
            permits.push(
                self.streams
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| Error::busy())?,
            );
        }
        if path.parts == ["api", "v1", "camera", "identify"] {
            permits.push(
                self.cameras
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| Error::busy())?,
            );
        }
        Ok(permits)
    }
    fn principal(&self, head: &Parts, image: bool, web: bool) -> Result<Principal> {
        if web {
            return Ok(Principal::PublicWeb);
        }
        if image {
            return Ok(Principal::PublicImage);
        }
        self.access
            .lock()
            .map_err(|_| Error::internal())?
            .authenticate(header_value(head, header::AUTHORIZATION)?)
    }
    fn headers(&self, head: &Parts) -> Result<()> {
        let host = header_value(head, header::HOST)?
            .ok_or_else(|| Error::new(403, "accessDenied", "HTTP authority is required."))?;
        if !self
            .configuration
            .authorities
            .contains(&host.to_ascii_lowercase())
            || head
                .uri
                .authority()
                .is_some_and(|a| !a.as_str().eq_ignore_ascii_case(host))
        {
            return Err(Error::new(
                403,
                "accessDenied",
                "Unrecognized HTTP authority.",
            ));
        }
        if header_value(head, header::ORIGIN)?.is_some_and(|v| v != self.configuration.origin) {
            return Err(Error::new(
                403,
                "accessDenied",
                "Cross-origin requests are not allowed.",
            ));
        }
        if head.headers.contains_key(header::COOKIE) {
            return Err(Error::params(
                "Cookies are not accepted; use a Bearer header.",
            ));
        }
        header_value(head, header::AUTHORIZATION)?;
        if header_value(head, header::CONTENT_ENCODING)?.is_some_and(|v| v != "identity") {
            return Err(Error::new(
                415,
                "invalidParams",
                "Compressed request bodies are not accepted.",
            ));
        }
        Ok(())
    }
    pub(crate) async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let request = serde_json::to_vec(
            &json!({"jsonrpc":"2.0","id":"http","method":format!("v1.{method}"),"params":params}),
        )
        .map_err(|_| Error::internal())?;
        let response = self.rpc.respond(&request).await;
        if let Some(e) = response.error {
            return Err(if e.code == -32602 {
                Error::params(&e.message)
            } else if e.code == -32601 {
                Error::new(404, "notFound", "Service method is not available.")
            } else {
                e.data.map(Error::from).unwrap_or_else(Error::internal)
            });
        }
        response.result.ok_or_else(Error::internal)
    }
}
async fn handle(State(gateway): State<Arc<Gateway>>, request: Request) -> Response {
    let mut response = gateway
        .respond(request)
        .await
        .unwrap_or_else(IntoResponse::into_response);
    let headers = response.headers_mut();
    if !headers.contains_key(header::CACHE_CONTROL) {
        headers.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    }
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    headers.insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
    if !headers.contains_key(header::CONTENT_SECURITY_POLICY) {
        headers.insert(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; frame-ancestors 'none'"
                .parse()
                .unwrap(),
        );
    }
    response
}
pub(crate) fn header_value(head: &Parts, key: header::HeaderName) -> Result<Option<&str>> {
    let mut values = head.headers.get_all(key).iter();
    let first = values.next();
    if values.next().is_some() {
        return Err(Error::params("Duplicate request header."));
    }
    first
        .map(|v| {
            v.to_str()
                .map_err(|_| Error::params("Invalid request header."))
        })
        .transpose()
}

async fn collect_body(head: &Parts, body: Body, limit: usize) -> Result<axum::body::Bytes> {
    let body = tokio::time::timeout(Duration::from_secs(5), to_bytes(body, limit))
        .await
        .map_err(|_| Error::new(408, "timeout", "Request body timed out."))?
        .map_err(|_| {
            Error::new(
                413,
                "resourceLimit",
                "Request body exceeds the route limit or cannot be read.",
            )
        })?;
    if !body.is_empty() {
        if !matches!(head.method.as_str(), "POST" | "PATCH") {
            return Err(Error::params("This method does not accept a body."));
        }
        if header_value(head, header::CONTENT_TYPE)?
            .and_then(|v| v.split(';').next())
            .is_none_or(|v| !v.trim().eq_ignore_ascii_case("application/json"))
        {
            return Err(Error::new(415, "invalidParams", "Use application/json."));
        }
    }
    Ok(body)
}
