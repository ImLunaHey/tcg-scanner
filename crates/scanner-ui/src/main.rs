#[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]
compile_error!("The Rune browser UI must be compiled without wasm atomics.");

mod api;
mod connection_view;
mod library;
mod operations;
mod state;
mod task;
mod view;

fn main() {
    rune::run(rune::AppSpec {
        #[cfg(not(target_arch = "wasm32"))]
        scenes: &[("saved", state::State::saved_scene)],
        setup: Some(state::State::setup),
        poll: Some(state::State::poll),
        ..rune::AppSpec::new("Scanner", (1180., 780.), state::State::default, view::view)
    });
}
