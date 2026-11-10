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
