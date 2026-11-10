use super::Command;
use crate::{api, connection_view::button, state::State, task::Task};
use rune::{
    El, col, row, text,
    tok::{self, type_},
};
use scanner_types::service::StorageStatus;
use serde_json::json;
#[derive(Default)]
pub struct Storage {
    pub task: Task<StorageStatus>,
    pub status: Option<StorageStatus>,
}
impl State {
    pub fn refresh_storage(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        if self.ops.storage.task.pending() {
            return;
        }
        self.ops
            .storage
            .task
            .request(async move { api::call(&client, "storage.status", json!({})).await });
    }
    pub fn reconnect_storage(&mut self) {
        self.submit(Command {
            method: "storage.reconnect",
            params: json!({}),
            job: None,
            focus: false,
        });
    }
    pub(crate) fn poll_storage(&mut self) -> bool {
        let Some(result) = self.ops.storage.task.take() else {
            return false;
        };
        match result {
            Ok(status) => self.ops.storage.status = Some(status),
            Err(error) => self.error = Some(error.to_string()),
        }
        true
    }
}
pub(super) fn panel(s: &State, t: tok::Theme) -> El {
    let status = s.ops.storage.status.as_ref();
    let description = status
        .map(|v| {
            let mut value = format!(
                "{}: {}",
                if v.available {
                    "Storage available"
                } else {
                    "Storage unavailable"
                },
                v.storage_root
            );
            if let Some(share) = &v.required_smb_share {
                value.push_str(&format!("\nRequired share: {share}"));
            }
            if let Some(error) = &v.error {
                value.push_str(&format!("\n{}", error.message));
            }
            value
        })
        .unwrap_or_else(|| "Checking storage…".into());
    col().gap(8.)
        .child(text(description).font(type_::BODY).color(t.text_secondary).wrap())
        .child(row().gap(8.)
            .child(button("Check storage", "check-storage", !s.ops.storage.task.pending(), t, State::refresh_storage))
            .child(button("Reopen reconnected storage", "reconnect-storage", !s.ops.busy(), t, State::reconnect_storage)))
        .child(text("Mount the configured share in your operating system first. Reopening requires an idle scanner and never moves files or resumes cancelled scans.").font(type_::LABEL).color(t.text_secondary).wrap())
}
