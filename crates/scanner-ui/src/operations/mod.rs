mod commands;
mod review;
mod settings;
mod settings_view;
pub use settings_view::{sidebar as settings_sidebar, view as settings_view};
mod view;
mod watch;
use crate::task::Task;
use scanner_types::{
    cards::CardLanguage,
    device::Device,
    identification::CardIdentification,
    recognition::{CardCondition, CardFinish},
    service::{ScanProfile, ServiceStatus},
};
use serde_json::Value;
use uuid::Uuid;
pub use view::{controls, review_panel};

pub struct Operations {
    pub devices: Vec<Device>,
    pub device: usize,
    pub profiles: Vec<ScanProfile>,
    pub profile: Option<usize>,
    pub duplex: bool,
    pub use_defaults: bool,
    pub settings: settings::Settings,
    pub language: CardLanguage,
    pub discovery: Task<Discovered>,
    pub mutation: Task<Value>,
    pub pending: Option<Command>,
    pub watch: Task<ServiceStatus>,
    pub review: Option<CardIdentification>,
    pub candidate: Option<String>,
    pub finish: CardFinish,
    pub condition: CardCondition,
}
pub struct Discovered {
    devices: Vec<Device>,
    profiles: Vec<ScanProfile>,
}
#[derive(Clone)]
pub struct Command {
    method: &'static str,
    params: Value,
    job: Option<Uuid>,
    focus: bool,
}
impl Default for Operations {
    fn default() -> Self {
        Self {
            devices: vec![],
            device: 0,
            profiles: vec![],
            profile: None,
            duplex: true,
            use_defaults: false,
            settings: Default::default(),
            language: CardLanguage::English,
            discovery: Task::default(),
            mutation: Task::default(),
            pending: None,
            watch: Task::default(),
            review: None,
            candidate: None,
            finish: CardFinish::Unverified,
            condition: CardCondition::Unassessed,
        }
    }
}
impl Operations {
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn set_waker(&mut self, waker: rune::Waker) {
        self.settings.inspection.set_waker(waker.clone());
        self.discovery.set_waker(waker.clone());
        self.mutation.set_waker(waker.clone());
        self.watch.set_waker(waker);
    }
    pub fn disconnect(&mut self) {
        self.settings.inspection.cancel();
        self.settings.report = None;
        self.settings.load(None);
        self.settings.profile_offset = 0;
        self.use_defaults = false;
        self.discovery.cancel();
        self.mutation.cancel();
        self.watch.cancel();
        self.pending = None;
        self.devices.clear();
        self.profiles.clear();
        self.profile = None;
        self.device = 0;
        self.review = None;
    }
}
