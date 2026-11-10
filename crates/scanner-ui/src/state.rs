use crate::{
    api::{self, Connected},
    library::{self, Card},
    task::Task,
};
use rune::{Image, ImageCache, TextEdit, Tx, Waker};
use scanner_client::Client;
use scanner_types::{
    error::Result,
    scan::ScanRecord,
    service::{JobPage, JobSummary, ServiceStatus},
};
use std::sync::Arc;

pub struct State {
    pub batches: Vec<JobSummary>,
    pub batch: usize,
    pub record: Option<ScanRecord>,
    pub cards: Vec<Card>,
    pub selected: usize,
    pub query: TextEdit,
    pub origin: TextEdit,
    pub token: TextEdit,
    pub error: Option<String>,
    pub image: Option<Image>,
    pub loading_image: bool,
    pub image_error: Option<String>,
    pub client: Option<Client>,
    pub status: Option<ServiceStatus>,
    pub offset: usize,
    pub next_offset: Option<usize>,
    pub connecting: Task<Connected>,
    pub listing: Task<JobPage>,
    pub details: Task<ScanRecord>,
    preview: Task<Vec<u8>>,
    cache: ImageCache,
    pub fixture: bool,
    pub ops: crate::operations::Operations,
    #[cfg(not(target_arch = "wasm32"))]
    fixtures: Vec<ScanRecord>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            batches: vec![],
            batch: 0,
            record: None,
            cards: vec![],
            selected: 0,
            query: TextEdit::new(""),
            origin: TextEdit::new(api::initial_origin()),
            token: TextEdit::new("").secure(),
            error: None,
            image: None,
            loading_image: false,
            image_error: None,
            client: None,
            status: None,
            offset: 0,
            next_offset: None,
            connecting: Task::default(),
            listing: Task::default(),
            details: Task::default(),
            preview: Task::default(),
            cache: ImageCache::new(Arc::new(|| {})),
            fixture: false,
            ops: Default::default(),
            #[cfg(not(target_arch = "wasm32"))]
            fixtures: vec![],
        }
    }
}
impl State {
    pub fn setup(&mut self, waker: Waker) {
        self.ops.set_waker(waker.clone());
        self.connecting.set_waker(waker.clone());
        self.listing.set_waker(waker.clone());
        self.details.set_waker(waker.clone());
        self.preview.set_waker(waker.clone());
        self.cache = ImageCache::new(waker);
        #[cfg(not(target_arch = "wasm32"))]
        self.setup_native();
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn setup_native(&mut self) {
        if self.fixture {
            return;
        }
        if std::env::var_os("SCANNER_UI_MANIFEST").is_some() {
            self.saved_scene();
            return;
        }
        if let Some(path) = std::env::var_os("SCANNER_TOKEN_FILE") {
            let origin = self.origin.text.clone();
            self.connecting.request(async move {
                let client = tokio::task::spawn_blocking(move || {
                    let token = scanner_platform::load_credential(std::path::Path::new(&path))?;
                    Client::new(&origin, &token)
                })
                .await
                .map_err(|_| {
                    scanner_types::error::ScannerError::new("task", "Credential loading failed.")
                })??;
                api::connect(client).await
            });
        }
    }
    pub fn connect(&mut self) {
        if self.connecting.pending() {
            return;
        }
        self.error = None;
        match Client::new(self.origin.text.trim(), self.token.text.trim()) {
            Ok(client) => {
                self.token = TextEdit::new("").secure();
                self.connecting.request(api::connect(client));
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    pub fn disconnect(&mut self) {
        if self.ops.mutation.pending() {
            return;
        }
        self.ops.disconnect();
        self.connecting.cancel();
        self.listing.cancel();
        self.details.cancel();
        self.preview.cancel();
        self.client = None;
        self.status = None;
        self.batches.clear();
        self.cards.clear();
        self.record = None;
        self.update_review();
        self.image = None;
        self.error = None;
        self.token = TextEdit::new("").secure();
    }
    pub fn refresh(&mut self, offset: usize) {
        if let Some(client) = self.client.clone() {
            self.offset = offset;
            self.listing.request(async move {
                api::call(
                    &client,
                    "jobs.list",
                    serde_json::json!({"limit":100,"offset":offset}),
                )
                .await
            });
        }
    }
    pub fn poll(&mut self, _: f64, _: &mut Tx) -> bool {
        let mut changed = self.poll_operations();
        if let Some(result) = self.connecting.take() {
            match result {
                Ok(connected) => {
                    self.client = Some(connected.client);
                    self.status = Some(connected.status);
                    self.offset = 0;
                    self.receive_page(connected.page);
                    self.error = None;
                    self.discover();
                    self.schedule_watch();
                }
                Err(error) => self.error = Some(error.to_string()),
            }
            changed = true;
        }
        if let Some(result) = self.listing.take() {
            match result {
                Ok(page) => self.receive_page(page),
                Err(error) => self.error = Some(error.to_string()),
            }
            changed = true;
        }
        if let Some(result) = self.details.take() {
            match result {
                Ok(record) => self.receive_record(record),
                Err(error) => self.error = Some(error.to_string()),
            }
            changed = true;
        }
        if let Some(result) = self.preview.take() {
            self.receive_preview(result);
            changed = true;
        }
        changed
    }
    fn receive_page(&mut self, page: JobPage) {
        let selected = self.batches.get(self.batch).map(|b| b.id);
        self.batches = page.jobs;
        self.next_offset = page.next_offset;
        let index = self
            .batches
            .iter()
            .position(|b| Some(b.id) == selected)
            .unwrap_or(0);
        self.select_batch(index);
    }
    pub(crate) fn receive_record(&mut self, record: ScanRecord) {
        let old_key = self.cards.get(self.selected).map(|c| c.source.key());
        let old_sequence = self.cards.get(self.selected).map(|c| c.sequence);
        let same_batch = self.record.as_ref().is_some_and(|r| r.id == record.id);
        if let Some(batch) = self.batches.iter_mut().find(|b| b.id == record.id) {
            *batch = (&record).into();
        }
        self.cards = library::cards(&record);
        self.record = Some(record);
        let selected = if same_batch {
            self.cards
                .iter()
                .position(|c| Some(c.sequence) == old_sequence)
                .unwrap_or(0)
        } else {
            0
        };
        if old_key.is_some() && old_key == self.cards.get(selected).map(|c| c.source.key()) {
            self.selected = selected;
            self.update_review();
        } else {
            self.select_card(selected);
        }
    }
    fn receive_preview(&mut self, result: Result<Vec<u8>>) {
        self.loading_image = false;
        match result {
            Ok(bytes) => self.image = Some(self.cache.load(bytes)),
            Err(error) => self.image_error = Some(error.to_string()),
        }
    }
    pub fn select_batch(&mut self, index: usize) {
        self.details.cancel();
        self.preview.cancel();
        self.cards.clear();
        self.record = None;
        self.update_review();
        self.image = None;
        self.batch = index;
        self.query = TextEdit::new("");
        self.loading_image = false;
        let Some(batch) = self.batches.get(index) else {
            return;
        };
        if let Some(client) = self.client.clone() {
            let id = batch.id;
            self.details.request(async move {
                api::call(&client, "jobs.get", serde_json::json!({"id":id})).await
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.fixture
            && let Some(record) = self.fixtures.get(index).cloned()
        {
            self.receive_record(record);
        }
    }
    pub fn select_card(&mut self, index: usize) {
        self.selected = index;
        self.update_review();
        self.image = None;
        self.image_error = None;
        self.preview.cancel();
        self.loading_image = false;
        let Some(card) = self.cards.get(index) else {
            return;
        };
        self.loading_image = true;
        let source = card.source.clone();
        if let Some(client) = self.client.clone() {
            self.preview
                .request(async move { api::thumbnail(&client, &source).await });
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.fixture {
            let source = card.source.clone();
            self.preview.request(async move {
                tokio::task::spawn_blocking(move || source.thumbnail())
                    .await
                    .map_err(|_| {
                        scanner_types::error::ScannerError::new("image", "Preview worker stopped.")
                    })?
            });
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn saved_scene(&mut self) {
        self.fixture = true;
        match library::read_fixture() {
            Ok(records) => {
                self.batches = records.iter().map(JobSummary::from).collect();
                self.fixtures = records;
                self.select_batch(0);
                self.preview.cancel();
                if let Some(card) = self.cards.get(self.selected) {
                    self.receive_preview(card.source.clone().thumbnail());
                }
                if let Some(image) = &self.image {
                    let _ = image.wait();
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}
