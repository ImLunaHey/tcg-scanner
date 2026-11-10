use crate::{api, state::State};
use scanner_types::identification::IdentificationState;

impl State {
    pub(crate) fn schedule_watch(&mut self) {
        if self.ops.watch.pending() {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        self.ops.watch.request(async move {
            #[cfg(not(target_arch = "wasm32"))]
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            #[cfg(target_arch = "wasm32")]
            gloo_timers::future::TimeoutFuture::new(3000).await;
            api::call(&client, "service.status", serde_json::json!({})).await
        });
    }
    pub(crate) fn poll_operations(&mut self) -> bool {
        let mut changed = false;
        if let Some(result) = self.ops.discovery.take() {
            match result {
                Ok(found) => {
                    self.ops.devices = found.devices;
                    self.ops.profiles = found.profiles;
                    self.ops.device = 0;
                    self.ops.profile = None;
                }
                Err(error) => self.error = Some(error.to_string()),
            }
            changed = true;
        }
        if let Some(result) = self.ops.mutation.take() {
            self.finish_command(result);
            changed = true;
        }
        if let Some(result) = self.ops.watch.take() {
            match result {
                Ok(status) => {
                    self.status = Some(status);
                    if self.record.as_ref().is_some_and(|r| {
                        !r.state.terminal()
                            || r.identifications().is_ok_and(|items| {
                                items
                                    .iter()
                                    .any(|i| i.state == IdentificationState::Processing)
                            })
                    }) {
                        self.refresh_record();
                    }
                    self.schedule_watch();
                }
                Err(error) => self.error = Some(error.to_string()),
            }
            changed = true;
        }
        changed
    }
}
