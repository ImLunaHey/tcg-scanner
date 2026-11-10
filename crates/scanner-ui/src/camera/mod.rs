#[cfg(target_arch = "wasm32")]
mod capture;
#[cfg(target_arch = "wasm32")]
mod state;
#[cfg(any(target_arch = "wasm32", test))]
mod steady;
#[cfg(target_arch = "wasm32")]
mod torch;
#[cfg(target_arch = "wasm32")]
mod view;
#[cfg(target_arch = "wasm32")]
pub use state::Camera;
#[cfg(target_arch = "wasm32")]
pub use view::view;

pub fn entry(t: rune::tok::Theme) -> Vec<rune::El> {
    #[cfg(target_arch = "wasm32")]
    {
        vec![
            rune::el()
                .role("link")
                .label("Open card camera")
                .href("/camera")
                .child(
                    rune::text("Open card camera")
                        .font(rune::tok::type_::LABEL)
                        .color(t.text),
                ),
        ]
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = t;
        vec![]
    }
}

#[cfg(target_arch = "wasm32")]
pub fn register_worker() {
    let Some(window) = web_sys::window() else {
        return;
    };
    if window.location().pathname().ok().as_deref() != Some("/camera")
        || !window.is_secure_context()
    {
        return;
    }
    let options = web_sys::RegistrationOptions::new();
    options.set_scope("/camera");
    options.set_type("module");
    let promise = window
        .navigator()
        .service_worker()
        .register_with_options("/camera-worker.js", &options);
    wasm_bindgen_futures::spawn_local(async move {
        // Identification remains usable if browser policy disables installation.
        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
    });
}
