#[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]
compile_error!("The Rune browser UI must be compiled without wasm atomics.");

mod api;
mod collection;
mod connection_view;
mod library;
mod operations;
mod reference_preview;
mod state;
mod task;
mod valuation;
mod view;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen(start))]
pub fn start() {
    rune::run(rune::AppSpec {
        #[cfg(not(target_arch = "wasm32"))]
        scenes: &[("saved", state::State::saved_scene)],
        setup: Some(state::State::setup),
        poll: Some(state::State::poll),
        ..rune::AppSpec::new("Scanner", (1180., 780.), state::State::default, view::view)
    });
}

mod review_link;
