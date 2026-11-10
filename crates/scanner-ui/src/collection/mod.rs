mod actions;
mod view;
use crate::task::Task;
use rune::{Image, ImageCache, TextEdit, Waker};
use scanner_types::{
    inventory::{InventoryMissingCards, InventoryPage},
    scan::ScanRecord,
};
use std::sync::Arc;
pub use view::{sidebar, view};

pub struct Collection {
    pub query: TextEdit,
    pub set: String,
    pub offset: usize,
    pub set_offset: usize,
    pub page: Option<InventoryPage>,
    pub listing: Task<(InventoryPage, usize)>,
    pub missing: Option<InventoryMissingCards>,
    pub missing_task: Task<InventoryMissingCards>,
    pub missing_mode: bool,
    pub missing_offset: usize,
    pub selected: usize,
    pub copy_offset: usize,
    pub preview: Task<Vec<u8>>,
    pub image: Option<Image>,
    pub image_error: Option<String>,
    pub error: Option<String>,
    pub opening: Task<ScanRecord>,
    cache: ImageCache,
}
impl Default for Collection {
    fn default() -> Self {
        Self {
            query: TextEdit::new(""),
            set: String::new(),
            offset: 0,
            set_offset: 0,
            page: None,
            listing: Task::default(),
            missing: None,
            missing_task: Task::default(),
            missing_mode: false,
            missing_offset: 0,
            selected: 0,
            copy_offset: 0,
            preview: Task::default(),
            image: None,
            image_error: None,
            error: None,
            opening: Task::default(),
            cache: ImageCache::new(Arc::new(|| {})),
        }
    }
}
impl Collection {
    pub fn set_waker(&mut self, waker: Waker) {
        self.listing.set_waker(waker.clone());
        self.missing_task.set_waker(waker.clone());
        self.preview.set_waker(waker.clone());
        self.opening.set_waker(waker.clone());
        self.cache = ImageCache::new(waker);
    }
    pub fn clear(&mut self) {
        self.listing.cancel();
        self.missing_task.cancel();
        self.preview.cancel();
        self.opening.cancel();
        self.page = None;
        self.missing = None;
        self.image = None;
        self.set.clear();
        self.missing_mode = false;
        self.offset = 0;
        self.set_offset = 0;
        self.missing_offset = 0;
        self.selected = 0;
        self.copy_offset = 0;
        self.query = TextEdit::new("");
        self.error = None;
        self.image_error = None;
    }
}
