#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod desktop;
use crate::{state::State, task::Task};
use rune::{
    El, Tx, col, row, text,
    tok::{self, type_},
};
use scanner_export::{Batch, Entry};
use scanner_types::error::{Result, ScannerError};
use uuid::Uuid;
#[cfg(target_arch = "wasm32")]
type Output = browser::Artifact;
#[cfg(not(target_arch = "wasm32"))]
type Output = String;
#[derive(Default)]
pub struct Exports {
    pub task: Task<Output>,
    pub notice: Option<String>,
    pub choosing: bool,
}
impl Exports {
    pub fn busy(&self) -> bool {
        self.choosing || self.task.pending()
    }
    pub fn clear(&mut self) {
        self.task.cancel();
        self.choosing = false;
        self.notice = None;
    }
}
#[derive(Clone, Copy)]
enum Kind {
    Manifest,
    Image,
    Original,
    Batch,
    Capabilities,
}
enum Request {
    Json { bytes: Vec<u8>, name: String },
    Image { job: Uuid, entry: Entry },
    Batch(Box<Batch>),
}
impl Request {
    fn name(&self) -> String {
        match self {
            Self::Json { name, .. } => name.clone(),
            Self::Image { entry, .. } => entry.name.clone(),
            Self::Batch(b) => format!("{}.zip", b.name()),
        }
    }
}
impl State {
    pub(crate) fn poll_export(&mut self) -> bool {
        let Some(result) = self.exports.task.take() else {
            return false;
        };
        #[cfg(target_arch = "wasm32")]
        let result = result.and_then(browser::save);
        self.exports.notice = Some(match result {
            Ok(message) => message,
            Err(error) => error.to_string(),
        });
        true
    }
    fn export(&mut self, kind: Kind, tx: &mut Tx) {
        if self.exports.busy() || self.ops.busy() {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        self.exports.notice = None;
        let request = match request(self, kind) {
            Ok(value) => value,
            Err(error) => {
                self.exports.notice = Some(error.to_string());
                return;
            }
        };
        #[cfg(target_arch = "wasm32")]
        {
            let _ = tx;
            self.exports
                .task
                .request(async move { browser::prepare(client, request).await });
        }
        #[cfg(not(target_arch = "wasm32"))]
        desktop::choose(self, tx, client, request);
    }
}
fn request(s: &State, kind: Kind) -> Result<Request> {
    if matches!(kind, Kind::Capabilities) {
        let report = s
            .ops
            .settings
            .report
            .as_ref()
            .ok_or_else(|| error("Inspect a scanner before exporting capabilities."))?;
        return Ok(Request::Json {
            bytes: serde_json::to_vec_pretty(report)
                .map_err(|_| error("Could not encode capabilities."))?,
            name: "scanner-capabilities.json".into(),
        });
    }
    let record = s
        .record
        .as_ref()
        .ok_or_else(|| error("Select a saved batch."))?;
    if matches!(kind, Kind::Manifest) {
        return Ok(Request::Json {
            bytes: serde_json::to_vec_pretty(record)
                .map_err(|_| error("Could not encode manifest."))?,
            name: format!("manifest-{}.json", record.batch_label()),
        });
    }
    let batch = Batch::new(record)?;
    if matches!(kind, Kind::Batch) {
        if !record.state.terminal() {
            return Err(error(
                "Wait for the batch to finish before exporting all images.",
            ));
        }
        return Ok(Request::Batch(Box::new(batch)));
    }
    let card = s
        .cards
        .get(s.selected)
        .ok_or_else(|| error("Select a saved image."))?;
    let edit = if matches!(kind, Kind::Original) {
        None
    } else {
        card.source.key().2
    };
    let entry = batch
        .entries()
        .iter()
        .find(|e| e.image.sequence == card.sequence && e.edit_id == edit)
        .ok_or_else(|| error("Selected image is no longer in this batch."))?
        .clone();
    Ok(Request::Image {
        job: record.id,
        entry,
    })
}
pub fn panel(s: &State, t: tok::Theme) -> El {
    let ready = s.client.is_some() && !s.ops.busy() && !s.exports.busy();
    let saved = ready && s.record.is_some();
    let images = saved && s.cards.get(s.selected).is_some();
    let controls = row()
        .gap(8.)
        .child(button("Export manifest", Kind::Manifest, saved, t))
        .child(button(
            "Export batch",
            Kind::Batch,
            saved && s.record.as_ref().is_some_and(|r| r.state.terminal()),
            t,
        ))
        .child(button("Export image", Kind::Image, images, t))
        .child(button("Export original", Kind::Original, images, t));
    let mut panel = col().gap(6.).child(controls);
    if s.exports.busy() {
        panel = panel.child(note("Preparing verified export…", t));
    }
    if let Some(message) = &s.exports.notice {
        panel = panel.child(note(message, t));
    }
    panel
}
pub fn capabilities(s: &State, t: tok::Theme) -> El {
    button(
        "Export capabilities",
        Kind::Capabilities,
        s.ops.settings.report.is_some() && !s.exports.busy(),
        t,
    )
}
fn button(label: &str, kind: Kind, enabled: bool, t: tok::Theme) -> El {
    let control = crate::connection_view::button(label, label, enabled, t, |_| {});
    if enabled {
        control.on_press_tx(move |s: &mut State, tx| s.export(kind, tx))
    } else {
        control
    }
}
fn note(value: &str, t: tok::Theme) -> El {
    text(value)
        .font(type_::LABEL)
        .color(t.text_secondary)
        .wrap()
}
fn error(message: &str) -> ScannerError {
    ScannerError::new("export", message)
}
