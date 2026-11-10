use super::Command;
use crate::{
    api,
    connection_view::button,
    state::State,
    task::{self, Task},
};
use rune::{
    El, col, text,
    tok::{self, type_},
};
use scanner_types::{
    scan::ScanState,
    staging::{TransferState, TransferStatus},
};

#[derive(Default)]
pub struct Transfer {
    pub data: Option<TransferStatus>,
    pub task: Task<TransferStatus>,
    pub error: Option<String>,
    pub link: Task<()>,
}
impl Transfer {
    pub fn clear(&mut self) {
        self.data = None;
        self.task.cancel();
        self.error = None;
        self.link.cancel();
    }
}
impl State {
    pub(crate) fn update_transfer(&mut self) {
        self.ops.transfer.clear();
        self.request_transfer(0);
    }
    pub fn refresh_transfer(&mut self) {
        self.request_transfer(0);
    }
    fn request_transfer(&mut self, delay: u32) {
        let (Some(client), Some(record)) = (self.client.clone(), self.record.as_ref()) else {
            return;
        };
        if self.ops.busy() {
            return;
        }
        let id = record.id;
        self.ops.transfer.error = None;
        self.ops.transfer.task.request(async move {
            if delay > 0 {
                task::delay(delay).await;
            }
            let result: TransferStatus =
                api::call(&client, "batches.storeStatus", serde_json::json!({"id":id})).await?;
            if result.job_id != id {
                return Err(scanner_types::error::ScannerError::new(
                    "protocol",
                    "Store status belongs to another batch.",
                ));
            }
            Ok(result)
        });
    }
    pub(crate) fn poll_transfer(&mut self) -> bool {
        let mut changed = false;
        if let Some(result) = self.ops.transfer.link.take() {
            if let Err(error) = result {
                self.ops.transfer.error = Some(error.to_string());
            }
            changed = true;
        }
        let Some(result) = self.ops.transfer.task.take() else {
            return changed;
        };
        match result {
            Ok(status) if self.record.as_ref().is_some_and(|r| r.id == status.job_id) => {
                let pending = status.state == TransferState::Sending;
                self.ops.transfer.data = Some(status);
                if pending {
                    self.request_transfer(2000);
                }
            }
            Ok(_) => {}
            Err(error) => self.ops.transfer.error = Some(error.to_string()),
        }
        true
    }
    fn send_to_store(&mut self) {
        if !can_send(self) {
            return;
        }
        let id = self.record.as_ref().unwrap().id;
        self.ops.transfer.clear();
        self.submit(Command {
            method: "batches.sendToStore",
            params: serde_json::json!({"id":id}),
            job: Some(id),
            focus: false,
        });
    }
}
fn can_send(s: &State) -> bool {
    !s.ops.busy()
        && s.client.is_some()
        && !s.fixture
        && s.record
            .as_ref()
            .is_some_and(|r| r.state == ScanState::Completed)
        && s.ops
            .transfer
            .data
            .as_ref()
            .is_some_and(|v| v.configured && v.state != TransferState::Sending)
}
pub fn panel(s: &State, t: tok::Theme) -> El {
    let mut panel = col()
        .gap(6.)
        .child(text("Store staging").font(type_::LABEL).color(t.text));
    if let Some(status) = &s.ops.transfer.data {
        let message = if !status.configured {
            "Store is not configured on the service.".into()
        } else {
            match status.state {
                TransferState::NotSent => "This batch has not been sent to Store.".into(),
                TransferState::Sending => format!(
                    "Sending: {} of {} cards acknowledged",
                    status.sent_cards, status.total_cards
                ),
                TransferState::Sent => format!(
                    "Sent: {} cards · {} preserved reviewer cards",
                    status.sent_cards,
                    status.preserved_cards.as_deref().unwrap_or_default().len()
                ),
                TransferState::Failed => format!(
                    "Transfer stopped: {}",
                    status
                        .error
                        .as_ref()
                        .map(|e| e.message.as_str())
                        .unwrap_or("Retry to resume.")
                ),
            }
        };
        panel = panel.child(note(message, t));
        if let Some(cards) = &status.preserved_cards
            && !cards.is_empty()
        {
            panel = panel.child(note(format!("Preserved: {}", cards.join(", ")), t));
        }
        if let Some(url) = status
            .review_url
            .as_deref()
            .and_then(crate::review_link::validated)
        {
            panel = panel.child(crate::review_link::control(url, t));
        }
    } else {
        panel = panel.child(note("Loading Store status…".into(), t));
    }
    if let Some(error) = &s.ops.transfer.error {
        panel = panel.child(note(error.clone(), t));
    }
    let label = match s.ops.transfer.data.as_ref().map(|v| v.state) {
        Some(TransferState::Failed) => "Retry Store transfer",
        Some(TransferState::Sent) => "Resync to Store",
        _ => "Send to Store",
    };
    panel
        .child(note(
            "Sends prepared front/back photos and review suggestions to Store for review.".into(),
            t,
        ))
        .child(button(
            label,
            "store-send",
            can_send(s),
            t,
            State::send_to_store,
        ))
        .child(button(
            "Refresh Store status",
            "store-refresh",
            s.client.is_some() && !s.ops.busy() && !s.ops.transfer.task.pending(),
            t,
            State::refresh_transfer,
        ))
}
fn note(value: String, t: tok::Theme) -> El {
    text(value)
        .font(type_::LABEL)
        .color(t.text_secondary)
        .wrap()
}
