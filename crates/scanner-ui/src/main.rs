mod library;
mod state;
mod view;

fn main() {
    rune::run(rune::AppSpec {
        scenes: &[("saved", state::State::saved_scene)],
        setup: Some(state::State::setup),
        poll: Some(state::State::poll),
        ..rune::AppSpec::new("Scanner", (1180., 780.), state::State::default, view::view)
    });
}
