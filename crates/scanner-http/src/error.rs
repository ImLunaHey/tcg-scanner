use axum::{
    Json,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use scanner_types::error::ScannerError;
use serde_json::json;

pub(crate) type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
pub(crate) struct Error {
    pub status: StatusCode,
    pub error: ScannerError,
}
impl Error {
    pub fn new(status: u16, kind: &str, message: &str) -> Self {
        Self {
            status: StatusCode::from_u16(status).expect("constant status"),
            error: ScannerError::new(kind, message),
        }
    }
    pub fn params(message: &str) -> Self {
        Self::new(400, "invalidParams", message)
    }
    pub fn busy() -> Self {
        Self::new(429, "resourceLimit", "Concurrent request limit reached.")
    }
    pub fn internal() -> Self {
        Self::new(
            500,
            "internal",
            "HTTP gateway could not complete the request.",
        )
    }
}
impl From<ScannerError> for Error {
    fn from(error: ScannerError) -> Self {
        let status = match error.kind.as_str() {
            "deviceNotFound" | "profileNotFound" | "imageNotFound" | "scanNotFound"
            | "notFound" | "settingsNotFound" => 404,
            "busy" | "conflict" | "ambiguousDevice" | "storeNotReady" | "storeConflict" => 409,
            "invalidParams"
            | "invalidSettings"
            | "unsupported"
            | "unsupportedSetting"
            | "unsupportedGame"
            | "unsupportedLanguage"
            | "cardNotFound" => 422,
            "accessDenied" => 403,
            "resourceLimit" => 429,
            "serviceUnavailable" | "transport" | "storage" | "storageUnavailable"
            | "storeNotConfigured" => 503,
            _ => 500,
        };
        Self {
            status: StatusCode::from_u16(status).unwrap(),
            error,
        }
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(json!({"schemaVersion":1,"error":self.error})),
        )
            .into_response();
        response
            .headers_mut()
            .insert(header::CONNECTION, "close".parse().unwrap());
        if self.status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                "Bearer realm=\"scanner\"".parse().unwrap(),
            );
        }
        if self.status == StatusCode::TOO_MANY_REQUESTS {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, "60".parse().unwrap());
        }
        response
    }
}
