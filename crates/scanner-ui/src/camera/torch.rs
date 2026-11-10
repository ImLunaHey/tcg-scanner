use super::capture::{error, js_error};
use scanner_types::error::Result;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::MediaStreamTrack;
pub fn supported(track: &MediaStreamTrack) -> bool {
    let Some(capabilities) = js_sys::Reflect::get(track, &"getCapabilities".into())
        .ok()
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
        .and_then(|f| f.call0(track).ok())
    else {
        return false;
    };
    js_sys::Reflect::get(&capabilities, &"torch".into())
        .ok()
        .and_then(|v| v.as_bool())
        == Some(true)
}
pub async fn set(track: MediaStreamTrack, on: bool) -> Result<bool> {
    let torch = js_sys::Object::new();
    js_sys::Reflect::set(&torch, &"torch".into(), &JsValue::from_bool(on)).map_err(js_error)?;
    let advanced = js_sys::Array::new();
    advanced.push(&torch);
    let constraints = js_sys::Object::new();
    js_sys::Reflect::set(&constraints, &"advanced".into(), &advanced).map_err(js_error)?;
    JsFuture::from(
        track
            .apply_constraints_with_constraints(constraints.unchecked_ref())
            .map_err(js_error)?,
    )
    .await
    .map_err(js_error)?;
    let actual = js_sys::Reflect::get(&track.get_settings(), &"torch".into())
        .map_err(js_error)?
        .as_bool();
    if actual != Some(on) {
        return Err(error(
            "The camera did not confirm the requested torch setting.",
        ));
    }
    Ok(on)
}
