//! Fetch uses explicit redirect refusal and a cancellable, bounded response stream.
use crate::transport;
use futures_util::{
    FutureExt,
    future::{Either, select},
};
use js_sys::{Reflect, Uint8Array};
use scanner_types::error::{Result, ScannerError};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AbortController, ReadableStreamDefaultReader, RequestInit, Response};

#[derive(Clone)]
pub(crate) struct Transport;
struct Abort(AbortController);
impl Drop for Abort {
    fn drop(&mut self) {
        self.0.abort();
    }
}
impl Transport {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
    pub async fn post(&self, endpoint: &url::Url, token: &str, body: Vec<u8>) -> Result<Vec<u8>> {
        let abort = Abort(AbortController::new().map_err(js_error)?);
        let request = request(endpoint, token, &body, &abort.0)?;
        let work = fetch(request).boxed_local();
        let timeout = gloo_timers::future::TimeoutFuture::new(120_000).boxed_local();
        match select(work, timeout).await {
            Either::Left((result, _)) => result,
            Either::Right(_) => Err(transport("Scanner request timed out.")),
        }
        // Abort also covers a dropped caller, failed validation and oversized bodies.
    }
}
fn request(
    endpoint: &url::Url,
    token: &str,
    body: &[u8],
    abort: &AbortController,
) -> Result<web_sys::Request> {
    let options = RequestInit::new();
    options.set_method("POST");
    options.set_body(&Uint8Array::from(body));
    options.set_signal(Some(&abort.signal()));
    options.set_redirect(web_sys::RequestRedirect::Error);
    options.set_credentials(web_sys::RequestCredentials::Omit);
    options.set_cache(web_sys::RequestCache::NoStore);
    options.set_referrer_policy(web_sys::ReferrerPolicy::NoReferrer);
    let request =
        web_sys::Request::new_with_str_and_init(endpoint.as_str(), &options).map_err(js_error)?;
    request
        .headers()
        .set("Authorization", token)
        .map_err(js_error)?;
    request
        .headers()
        .set("Content-Type", "application/json")
        .map_err(js_error)?;
    Ok(request)
}
async fn fetch(request: web_sys::Request) -> Result<Vec<u8>> {
    let window = web_sys::window().ok_or_else(|| transport("Browser window is unavailable."))?;
    let response: Response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(js_error)?
        .dyn_into()
        .map_err(js_error)?;
    if !response.ok() {
        return Err(transport(format!(
            "Scanner HTTP request failed with status {}.",
            response.status()
        )));
    }
    let length = response.headers().get("Content-Length").map_err(js_error)?;
    if length
        .and_then(|v| v.parse::<u64>().ok())
        .is_some_and(|n| n > 16 * 1024 * 1024)
    {
        return Err(limit());
    }
    let stream = response
        .body()
        .ok_or_else(|| transport("Missing service response body."))?;
    let reader: ReadableStreamDefaultReader = stream.get_reader().dyn_into().map_err(js_error)?;
    collect(reader).await
}
async fn collect(reader: ReadableStreamDefaultReader) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    loop {
        let part = JsFuture::from(reader.read()).await.map_err(js_error)?;
        if Reflect::get(&part, &JsValue::from_str("done"))
            .map_err(js_error)?
            .as_bool()
            == Some(true)
        {
            break;
        }
        let chunk: Uint8Array = Reflect::get(&part, &JsValue::from_str("value"))
            .map_err(js_error)?
            .dyn_into()
            .map_err(js_error)?;
        if bytes.len() + chunk.length() as usize > 16 * 1024 * 1024 {
            return Err(limit());
        }
        let offset = bytes.len();
        bytes.resize(offset + chunk.length() as usize, 0);
        chunk.copy_to(&mut bytes[offset..]);
    }
    reader.release_lock();
    Ok(bytes)
}
fn limit() -> ScannerError {
    ScannerError::new("resourceLimit", "Service response exceeds 16 MiB.")
}
fn js_error(_: impl Into<JsValue>) -> ScannerError {
    // Browser exception text can include request details; never propagate credentials.
    transport("Browser could not complete the scanner request.")
}
