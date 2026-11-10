use super::{
    capture::{Capture, Control},
    steady::{Observation, SteadyCard},
    torch,
};
use crate::task::Task;
use rune::{Image, ImageCache, Waker};
use scanner_client::CameraClient;
use scanner_types::{
    camera::{CameraIdentification, CameraRequest, CameraState},
    cards::CardLanguage,
};
use std::sync::Arc;
pub struct Camera {
    pub enabled: bool,
    pub language: CardLanguage,
    pub status: String,
    pub error: Option<String>,
    pub live: Option<Image>,
    pub preview: Option<Image>,
    pub result: Option<CameraIdentification>,
    pub details: bool,
    pub opening: Task<Capture>,
    pub lookup: Task<(CameraIdentification, Image)>,
    pub torch_task: Task<bool>,
    pub torch_supported: bool,
    pub torch_on: bool,
    capture: Option<Capture>,
    control: Control,
    timer: Task<()>,
    gate: SteadyCard,
    cache: ImageCache,
    wake: Waker,
}
impl Default for Camera {
    fn default() -> Self {
        let wake: Waker = Arc::new(|| {});
        Self {
            enabled: web_sys::window()
                .and_then(|w| w.location().pathname().ok())
                .is_some_and(|p| p == "/camera"),
            language: CardLanguage::English,
            status: "Place one card front inside the frame.".into(),
            error: None,
            live: None,
            preview: None,
            result: None,
            details: false,
            opening: Default::default(),
            lookup: Default::default(),
            torch_task: Default::default(),
            torch_supported: false,
            torch_on: false,
            capture: None,
            control: Default::default(),
            timer: Default::default(),
            gate: Default::default(),
            cache: ImageCache::new(wake.clone()),
            wake,
        }
    }
}
impl Camera {
    pub fn setup(&mut self, wake: Waker) {
        self.wake = wake.clone();
        self.cache = ImageCache::new(wake.clone());
        self.opening.set_waker(wake.clone());
        self.lookup.set_waker(wake.clone());
        self.torch_task.set_waker(wake.clone());
        self.timer.set_waker(wake);
    }
    pub fn active(&self) -> bool {
        self.capture.is_some() || self.opening.pending()
    }
    pub fn start(&mut self) {
        if self.active() {
            return;
        }
        self.reset();
        self.error = None;
        self.control = Control::default();
        self.status = "Allow camera access if prompted.".into();
        self.opening
            .request(Capture::open(self.control.clone(), self.wake.clone()));
    }
    pub fn stop(&mut self) {
        self.control.stop();
        self.opening.cancel();
        self.lookup.cancel();
        self.torch_task.cancel();
        self.timer.cancel();
        self.capture = None;
        self.live = None;
        self.torch_supported = false;
        self.torch_on = false;
        self.status = "Camera stopped. Start again when ready.".into();
    }
    pub fn reset(&mut self) {
        self.lookup.cancel();
        self.result = None;
        self.preview = None;
        self.details = false;
        self.gate = Default::default();
    }
    pub fn language(&mut self, language: CardLanguage) {
        let active = self.active();
        self.stop();
        self.reset();
        self.language = language;
        if active {
            self.start();
        }
    }
    pub fn toggle_torch(&mut self) {
        if self.torch_task.pending() {
            return;
        }
        if let Some(track) = self.capture.as_ref().and_then(Capture::track) {
            self.torch_task.request(torch::set(track, !self.torch_on));
        }
    }
    pub fn show_details(&mut self, show: bool) {
        self.details = show;
        if show {
            self.lookup.cancel();
        }
    }
    pub fn poll(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        if self.active() && self.control.stopped() {
            self.stop();
            return true;
        }
        let mut changed = false;
        if let Some(result) = self.opening.take() {
            changed = true;
            match result {
                Ok(capture) => {
                    self.torch_supported = capture.track().is_some_and(|t| torch::supported(&t));
                    self.capture = Some(capture);
                    self.status = "Keep all four corners visible.".into();
                    self.schedule();
                }
                Err(error) => {
                    self.stop();
                    self.error = Some(error.message);
                }
            }
        }
        if let Some(result) = self.lookup.take() {
            changed = true;
            self.apply(result);
        }
        if let Some(result) = self.torch_task.take() {
            changed = true;
            match result {
                Ok(on) => self.torch_on = on,
                Err(error) => self.error = Some(error.message),
            }
        }
        if self.timer.take().is_some() {
            changed = true;
            if !self.details
                && let Err(error) = self.tick()
            {
                self.stop();
                self.error = Some(error.message);
            }
            if self.capture.is_some() {
                self.schedule();
            }
        }
        changed
    }
    fn schedule(&mut self) {
        self.timer.request(async {
            crate::task::delay(200).await;
            Ok(())
        });
    }
    fn tick(&mut self) -> scanner_types::error::Result<()> {
        let Some(capture) = self.capture.as_ref() else {
            return Ok(());
        };
        if capture
            .track()
            .is_none_or(|t| t.ready_state() == web_sys::MediaStreamTrackState::Ended)
        {
            return Err(super::capture::error(
                "Camera disconnected. Start it again when ready.",
            ));
        }
        let Some(frame) = capture.frame()? else {
            return Ok(());
        };
        let image = self.cache.rgba(240, 320, frame.rgba);
        self.live = Some(image.clone());
        if self.lookup.pending() {
            return Ok(());
        }
        let now = js_sys::Date::now();
        match self.gate.observe(&frame.sample, now) {
            Observation::Ready => {
                let base64 = capture.jpeg()?;
                let origin = web_sys::window()
                    .ok_or_else(|| super::capture::error("Browser is unavailable."))?
                    .location()
                    .origin()
                    .map_err(super::capture::js_error)?;
                let client = CameraClient::new(&origin)?;
                let language = self.language;
                self.gate.accept(frame.sample, now);
                self.error = None;
                self.status = "Checking card identity and finish…".into();
                self.lookup.request(async move {
                    Ok((
                        client.identify(CameraRequest { base64, language }).await?,
                        image,
                    ))
                });
            }
            Observation::Searching => self.status = "Place one card inside the frame.".into(),
            Observation::Moving => {
                self.status = "Hold steady with all four corners visible.".into()
            }
            Observation::Unchanged => {
                self.status = "Card already checked. Move to the next card.".into()
            }
            Observation::Waiting => {}
        }
        Ok(())
    }
    fn apply(&mut self, result: scanner_types::error::Result<(CameraIdentification, Image)>) {
        match result {
            Ok((value, preview)) => {
                self.gate.matched =
                    matches!(value.state, CameraState::Matched | CameraState::CodeCard);
                self.status = match value.state {
                    CameraState::Matched => format!(
                        "{} · ready for the next card",
                        value
                            .candidates
                            .first()
                            .map(|c| c.name.as_str())
                            .unwrap_or("Card found")
                    ),
                    CameraState::NoCard => "Keep all four corners inside the frame.".into(),
                    CameraState::CardBack => "Turn the card over to show its front.".into(),
                    CameraState::CodeCard => "Pokémon LIVE code card.".into(),
                    CameraState::Ambiguous => {
                        "Several possible matches. Try closer, with less glare.".into()
                    }
                    CameraState::NoMatch if !value.visual_matches.is_empty()=>"Similar artwork found. Check the printed details before choosing a printing.".into(),
                    CameraState::NoMatch if value.energy_symbol.is_some()=>"Energy type found. Exact printing is unknown.".into(),
                    CameraState::NoMatch => "No match yet. Try closer, with less glare.".into(),
                };
                if !value.candidates.is_empty()
                    || !value.visual_matches.is_empty()
                    || value.uncatalogued_card.is_some()
                    || value.energy_symbol.is_some()
                    || value.state == CameraState::CodeCard
                {
                    self.result = Some(value);
                    self.preview = Some(preview);
                }
            }
            Err(error) => {
                if error.kind == "transport" {
                    self.stop();
                } else {
                    self.gate.retry_after(
                        js_sys::Date::now(),
                        if error.kind == "rateLimit" {
                            60000.
                        } else {
                            2500.
                        },
                    );
                    self.status = "Hold steady to retry.".into();
                }
                self.error = Some(error.message);
            }
        }
    }
}
