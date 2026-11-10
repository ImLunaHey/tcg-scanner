use wasm_bindgen::{JsCast, JsValue, prelude::*};
use wasm_bindgen_futures::{JsFuture, future_to_promise};
use web_sys::{
    ExtendableEvent, FetchEvent, RequestMode, Response, ResponseInit, ServiceWorkerGlobalScope,
};

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let scope: ServiceWorkerGlobalScope = js_sys::global().dyn_into()?;
    let install_scope = scope.clone();
    let install = Closure::<dyn FnMut(ExtendableEvent)>::new(move |event: ExtendableEvent| {
        if let Ok(promise) = install_scope.skip_waiting() {
            let _ = event.wait_until(&promise);
        }
    });
    scope.add_event_listener_with_callback("install", install.as_ref().unchecked_ref())?;
    install.forget();
    let activate_scope = scope.clone();
    let activate = Closure::<dyn FnMut(ExtendableEvent)>::new(move |event: ExtendableEvent| {
        let _ = event.wait_until(&activate_scope.clients().claim());
    });
    scope.add_event_listener_with_callback("activate", activate.as_ref().unchecked_ref())?;
    activate.forget();
    let fetch_scope = scope.clone();
    let fetch = Closure::<dyn FnMut(FetchEvent)>::new(move |event: FetchEvent| {
        let request = event.request();
        if request.mode() != RequestMode::Navigate || request.method() != "GET" {
            return;
        }
        let Ok(url) = web_sys::Url::new(&request.url()) else {
            return;
        };
        if url.pathname() != "/camera" {
            return;
        }
        let network = fetch_scope.fetch_with_request(&request);
        let promise = future_to_promise(async move {
            match JsFuture::from(network).await {
                Ok(response) => Ok(response),
                Err(_) => offline().map(JsValue::from),
            }
        });
        let _ = event.respond_with(&promise);
    });
    scope.add_event_listener_with_callback("fetch", fetch.as_ref().unchecked_ref())?;
    fetch.forget();
    Ok(())
}
fn offline() -> Result<Response, JsValue> {
    let options = ResponseInit::new();
    options.set_status(503);
    let headers = web_sys::Headers::new()?;
    headers.set("content-type", "text/html; charset=utf-8")?;
    headers.set("cache-control", "no-store")?;
    headers.set(
        "content-security-policy",
        "default-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'",
    )?;
    options.set_headers(headers.as_ref());
    Response::new_with_opt_str_and_init(Some(OFFLINE), &options)
}
const OFFLINE: &str = "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Scanner is offline</title><h1>Scanner is offline</h1><p>Reconnect to the scanner service and Tailscale, then reload. Card identification needs the service online.</p><p>No camera frames or card results are saved for offline use.</p><a href=\"/camera\">Try again</a></html>";
