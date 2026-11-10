use crate::{api, connection_view::button, library::ImageSource, state::State, task::Task};
use rune::{
    El, Image, ImageCache, SurfaceFit, col, el, text,
    tok::{self, type_},
};
use std::sync::Arc;
type Key = (uuid::Uuid, u32, Option<uuid::Uuid>, String);
pub struct Comparison {
    primary: Option<Key>,
    source: Option<Key>,
    image: Option<Image>,
    error: Option<String>,
    task: Task<Vec<u8>>,
    cache: ImageCache,
}
impl Default for Comparison {
    fn default() -> Self {
        Self {
            primary: None,
            source: None,
            image: None,
            error: None,
            task: Default::default(),
            cache: ImageCache::new(Arc::new(|| {})),
        }
    }
}
impl Comparison {
    pub fn setup(&mut self, waker: rune::Waker) {
        self.task.set_waker(waker.clone());
        self.cache = ImageCache::new(waker);
    }
    pub fn clear(&mut self) {
        self.task.cancel();
        self.primary = None;
        self.source = None;
        self.image = None;
        self.error = None;
    }
    pub fn active(&self) -> bool {
        self.source.is_some()
    }
    pub fn poll(&mut self) -> bool {
        let Some(result) = self.task.take() else {
            return false;
        };
        match result {
            Ok(bytes) => self.image = Some(self.cache.load(bytes)),
            Err(error) => self.error = Some(error.to_string()),
        };
        true
    }
}
fn other(s: &State) -> Option<ImageSource> {
    if !s.record.as_ref()?.request.duplex {
        return None;
    }
    let current = s.cards.get(s.selected)?.sequence;
    let target = if current % 2 == 0 {
        current.checked_sub(1)?
    } else {
        current.checked_add(1)?
    };
    s.cards
        .iter()
        .find(|c| c.sequence == target)
        .map(|c| c.source.clone())
}
pub fn sync(s: &mut State) {
    if s.comparison.active()
        && (s.cards.get(s.selected).map(|c| c.source.key()) != s.comparison.primary
            || other(s).map(|c| c.key()) != s.comparison.source)
    {
        s.comparison.clear();
    }
}
fn toggle(s: &mut State) {
    if s.comparison.active() {
        s.comparison.clear();
        return;
    }
    let Some(source) = other(s) else {
        return;
    };
    s.comparison.primary = s.cards.get(s.selected).map(|c| c.source.key());
    s.comparison.source = Some(source.key());
    if let Some(client) = s.client.clone() {
        s.comparison
            .task
            .request(async move { api::thumbnail(&client, &source).await });
    }
    #[cfg(not(target_arch = "wasm32"))]
    if s.fixture {
        let Some(source) = other(s) else {
            return;
        };
        s.comparison.task.request(async move {
            tokio::task::spawn_blocking(move || source.thumbnail())
                .await
                .map_err(|e| scanner_types::error::ScannerError::new("image", e.to_string()))?
        });
    }
}
pub fn control(s: &State, t: tok::Theme) -> El {
    button(
        if s.comparison.active() {
            "Hide other side"
        } else {
            "Compare other side"
        },
        "compare-side",
        other(s).is_some() && s.ops.editor.is_none(),
        t,
        toggle,
    )
}
pub fn view(s: &State, t: tok::Theme) -> El {
    let c = &s.comparison;
    let mut image = el()
        .grow(1.)
        .min_h(120.)
        .min_w(0.)
        .w_full()
        .radius(12.)
        .fill(t.card);
    if let Some(pixels) = &c.image {
        image = image.pixels_fit(pixels.pixels(), SurfaceFit::Fit);
    } else {
        image = image.center().child(
            text(c.error.as_deref().unwrap_or("Loading other side…"))
                .font(type_::LABEL)
                .wrap()
                .color(t.text_secondary),
        );
    }
    col()
        .grow(1.)
        .min_w(0.)
        .h_full()
        .gap(4.)
        .child(
            text("Other side · same physical card")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scanner_types::{
        device::Device,
        scan::{ScanRecord, ScanRequest},
    };
    #[test]
    fn comparison_stays_with_the_duplex_pair_and_invalidates_changed_sources() {
        let mut record = ScanRecord::new(
            Device::default(),
            ScanRequest {
                duplex: true,
                ..Default::default()
            },
            "/fixture".into(),
        );
        record.images=(1..=3).map(|i|serde_json::from_value(serde_json::json!({"sequence":i,"relativePath":format!("{i}.png"),"bytes":100,"sha256":format!("hash-{i}"),"width":30,"height":40,"imageCount":1,"uniformType":"public.png","side":"unknown"})).unwrap()).collect();
        let mut state = State::default();
        state.receive_record(record.clone());
        assert_eq!(other(&state).unwrap().key().1, 2);
        toggle(&mut state);
        assert!(state.comparison.active());
        record.images[1].sha256 = "changed".into();
        state.receive_record(record.clone());
        sync(&mut state);
        assert!(!state.comparison.active());
        state.select_card(1);
        assert_eq!(other(&state).unwrap().key().1, 1);
        state.select_card(2);
        assert!(other(&state).is_none());
        record.request.duplex = false;
        state.receive_record(record);
        state.select_card(0);
        assert!(other(&state).is_none());
    }
}
