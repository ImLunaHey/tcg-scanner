use crate::library::{self, Card};
use rune::{Image, ImageCache, Latest, TextEdit, Tx, Waker};
use scanner_types::{error::Result, scan::ScanRecord};
use std::sync::Arc;

pub struct State {
    pub batches: Vec<ScanRecord>,
    pub batch: usize,
    pub cards: Vec<Card>,
    pub selected: usize,
    pub query: TextEdit,
    pub error: Option<String>,
    pub image: Option<Image>,
    pub loading_image: bool,
    pub image_error: Option<String>,
    cache: ImageCache,
    library: Latest<Result<Vec<ScanRecord>>>,
    preview: Latest<Result<Vec<u8>>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            batches: vec![],
            batch: 0,
            cards: vec![],
            selected: 0,
            query: TextEdit::new(""),
            error: None,
            image: None,
            loading_image: false,
            image_error: None,
            cache: ImageCache::new(Arc::new(|| {})),
            library: Latest::default(),
            preview: Latest::default(),
        }
    }
}

impl State {
    pub fn setup(&mut self, waker: Waker) {
        self.library.set_waker(waker.clone());
        self.preview.set_waker(waker);
        if self.batches.is_empty() && self.error.is_none() {
            self.library.request((), |_| library::read_library());
        }
    }
    fn receive_library(&mut self, result: Result<Vec<ScanRecord>>) {
        match result {
            Ok(records) => {
                self.batches = records;
                self.error = None;
                self.select_batch(0);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    fn receive_preview(&mut self, result: Result<Vec<u8>>) {
        self.loading_image = false;
        match result {
            Ok(bytes) => self.image = Some(self.cache.load(bytes)),
            Err(error) => self.image_error = Some(error.to_string()),
        }
    }
    pub fn poll(&mut self, _: f64, _: &mut Tx) -> bool {
        let mut changed = false;
        if let Some(result) = self.library.take() {
            self.receive_library(result);
            changed = true;
        }
        if let Some(result) = self.preview.take() {
            self.receive_preview(result);
            changed = true;
        }
        changed
    }
    pub fn select_batch(&mut self, index: usize) {
        let Some(record) = self.batches.get(index) else {
            return;
        };
        self.batch = index;
        self.query = TextEdit::new("");
        self.cards = library::cards(record);
        self.select_card(0);
    }
    pub fn select_card(&mut self, index: usize) {
        self.selected = index;
        self.image = None;
        self.image_error = None;
        self.preview.cancel();
        self.loading_image = false;
        if let Some(card) = self.cards.get(index) {
            self.loading_image = true;
            self.preview
                .request(card.source.clone(), library::ImageSource::thumbnail);
        }
    }
    pub fn saved_scene(&mut self) {
        // Only deterministic headless scenes wait; interactive reads use Latest workers.
        self.receive_library(library::read_library());
        self.preview.cancel();
        if let Some(card) = self.cards.get(self.selected) {
            self.receive_preview(card.source.clone().thumbnail());
        }
        if let Some(image) = &self.image {
            let _ = image.wait();
        }
    }
}
