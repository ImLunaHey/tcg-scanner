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
                Ok(found) => self.update_discovery(found),
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
    fn update_discovery(&mut self, found: super::Discovered) {
        let device = self.ops.devices.get(self.ops.device).map(|d| d.id.clone());
        let profile = self
            .ops
            .profile
            .and_then(|i| self.ops.profiles.get(i))
            .map(|p| p.id);
        self.ops.device = found
            .devices
            .iter()
            .position(|d| Some(&d.id) == device.as_ref())
            .unwrap_or(0);
        self.ops.profile = found.profiles.iter().position(|p| Some(p.id) == profile);
        if self.ops.settings.report.as_ref().is_some_and(|r| {
            found
                .devices
                .get(self.ops.device)
                .is_none_or(|d| d.id != r.device.id)
        }) {
            self.ops.settings.report = None;
        }
        self.ops.devices = found.devices;
        self.ops.profiles = found.profiles;
    }
}
