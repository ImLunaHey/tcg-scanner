use super::{Command, Discovered};
use crate::{api, state::State};
use scanner_types::{
    error::Result,
    scan::{ScanRecord, ScanRequest},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

impl State {
    pub fn discover(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        if self.ops.discovery.pending() {
            return;
        }
        self.ops.discovery.request(async move {
            #[derive(Deserialize)]
            struct Devices {
                devices: Vec<scanner_types::device::Device>,
            }
            #[derive(Deserialize)]
            struct Profiles {
                profiles: Vec<scanner_types::service::ScanProfile>,
            }
            let devices: Devices = api::call(&client, "devices", json!({})).await?;
            let profiles: Profiles = api::call(&client, "profiles.list", json!({})).await?;
            Ok(Discovered {
                devices: devices.devices,
                profiles: profiles.profiles,
            })
        });
    }
    fn scan_submission(&self) -> Option<ScanRequest> {
        let device = self.ops.devices.get(self.ops.device)?;
        let mut request = ScanRequest {
            device_id: Some(device.id.clone()),
            duplex: self.ops.duplex,
            prepare_cards: Some(true),
            card_language: Some(self.ops.language),
            use_device_settings: self.ops.use_defaults.then_some(true),
            ..ScanRequest::default()
        };
        if let Some(profile) = self.ops.profile.and_then(|i| self.ops.profiles.get(i)) {
            request.profile_id = Some(profile.id);
            request.use_device_settings = None;
            profile.settings.apply(&mut request);
        }
        Some(request)
    }
    pub fn toggle_pause(&mut self) {
        let Some(status) = &self.status else {
            return;
        };
        self.submit(Command {
            method: if status.paused {
                "service.resume"
            } else {
                "service.pause"
            },
            params: json!({}),
            job: None,
            focus: false,
        });
    }
    pub fn start_scan(&mut self) {
        let Some(request) = self.scan_submission() else {
            return;
        };
        self.submit(Command {
            method: "scan.start",
            params: json!(request),
            job: None,
            focus: true,
        });
    }
    pub fn cancel_scan(&mut self) {
        let Some(record) = &self.record else {
            return;
        };
        if record.state.terminal() {
            return;
        }
        self.submit(Command {
            method: "jobs.cancel",
            params: json!({"id":record.id}),
            job: Some(record.id),
            focus: false,
        });
    }
    pub fn continue_batch(&mut self) {
        let Some(record) = &self.record else {
            return;
        };
        self.submit(Command {
            method: "batches.continue",
            params: json!({"id":record.id,"requestID":Uuid::new_v4()}),
            job: Some(record.id),
            focus: true,
        });
    }
    pub fn prepare_card(&mut self) {
        self.card_command("images.prepareCard");
    }
    pub fn identify_card(&mut self) {
        self.card_command("images.identify");
    }
    fn card_command(&mut self, method: &'static str) {
        let (Some(record), Some(card)) = (&self.record, self.cards.get(self.selected)) else {
            return;
        };
        self.submit(Command {
            method,
            params: json!({"id":Uuid::new_v4(),"jobID":record.id,"sequence":card.sequence}),
            job: Some(record.id),
            focus: false,
        });
    }
    pub(super) fn submit(&mut self, command: Command) {
        if self.ops.busy() || self.client.is_none() || self.fixture {
            return;
        }
        self.valuation.clear();
        self.ops.pending = Some(command);
        self.retry_command();
    }
    pub fn retry_command(&mut self) {
        if self.ops.mutation.pending() {
            return;
        }
        let (Some(client), Some(command)) = (self.client.clone(), self.ops.pending.clone()) else {
            return;
        };
        self.error = None;
        self.ops
            .mutation
            .request(async move { client.call(command.method, command.params).await });
    }
    pub(super) fn finish_command(&mut self, result: Result<Value>) {
        let Some(command) = self.ops.pending.clone() else {
            return;
        };
        match result {
            Ok(value) => {
                self.ops.pending = None;
                self.error = None;
                self.finish_settings(&command, &value);
                if matches!(command.method, "service.pause" | "service.resume")
                    && let Ok(status) = serde_json::from_value(value.clone())
                {
                    self.status = Some(status);
                }
                if command.focus {
                    match serde_json::from_value::<ScanRecord>(value) {
                        Ok(record) => {
                            self.focus_record(record);
                        }
                        Err(_) => {
                            self.error = Some(
                                "The service accepted the request; refresh to see the saved job."
                                    .into(),
                            );
                            self.refresh(0);
                        }
                    }
                } else if self.record.as_ref().map(|r| r.id) == command.job {
                    self.details.cancel();
                    self.refresh_record();
                }
                self.schedule_watch();
            }
            Err(error) => {
                let uncertain = matches!(error.kind.as_str(), "transport" | "task" | "storage");
                self.error = Some(if uncertain {
                    format!("{error} Retry reuses the same request.")
                } else {
                    error.to_string()
                });
                if !uncertain {
                    self.ops.pending = None;
                }
            }
        }
    }
    pub(crate) fn focus_record(&mut self, record: ScanRecord) {
        if let Some(i) = self.batches.iter().position(|b| b.id == record.id) {
            self.batches[i] = (&record).into();
            self.batch = i;
        } else {
            self.batches.insert(0, (&record).into());
            self.batch = 0;
        }
        self.details.cancel();
        self.receive_record(record);
    }
    pub fn refresh_record(&mut self) {
        let (Some(client), Some(record)) = (self.client.clone(), &self.record) else {
            return;
        };
        if self.details.pending() {
            return;
        }
        let id = record.id;
        self.details
            .request(async move { api::call(&client, "jobs.get", json!({"id":id})).await });
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use crate::{state::State, task::Task};
    use scanner_client::Client;
    use scanner_types::device::Device;
    use scanner_types::{
        error::Result,
        scan::{ScanRecord, ScanRequest},
    };
    use serde_json::{Value, json};
    use std::{sync::Arc, time::Duration};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::{Notify, mpsc},
    };

    async fn read_request(socket: &mut TcpStream) -> Value {
        let mut bytes = Vec::new();
        let split = loop {
            let mut chunk = [0; 4096];
            let size = socket.read(&mut chunk).await.unwrap();
            assert!(size > 0 && bytes.len() < 65536);
            bytes.extend_from_slice(&chunk[..size]);
            if let Some(i) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                break i + 4;
            }
        };
        let headers = std::str::from_utf8(&bytes[..split]).unwrap();
        let length: usize = headers
            .lines()
            .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse().unwrap())
            })
            .unwrap();
        assert!(length <= 65536);
        while bytes.len() < split + length {
            let mut chunk = [0; 4096];
            let size = socket.read(&mut chunk).await.unwrap();
            assert!(size > 0);
            bytes.extend_from_slice(&chunk[..size]);
        }
        serde_json::from_slice(&bytes[split..split + length]).unwrap()
    }
    async fn reply(socket: &mut TcpStream, id: &Value, result: Value) {
        let body = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":id,"result":result})).unwrap();
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        socket.write_all(headers.as_bytes()).await.unwrap();
        socket.write_all(&body).await.unwrap();
    }
    async fn result<T>(task: &mut Task<T>, notify: &Notify) -> Result<T> {
        tokio::time::timeout(Duration::from_secs(5), notify.notified())
            .await
            .unwrap();
        task.take()
            .expect("The completion waker must publish a result")
    }
    #[test]
    fn explicit_profiles_override_defaults_without_combining_both_modes() {
        use scanner_types::{
            scan::OriginalFormat,
            service::{ScanProfile, ScanSettings},
        };
        let mut state = State::default();
        state.ops.devices.push(Device {
            id: "fixture".into(),
            ..Device::default()
        });
        state.ops.use_defaults = true;
        assert_eq!(
            state.scan_submission().unwrap().use_device_settings,
            Some(true)
        );
        let profile = ScanProfile {
            id: uuid::Uuid::new_v4(),
            name: "Duplex TIFF".into(),
            settings: ScanSettings {
                dpi: 300,
                duplex: true,
                format: OriginalFormat::Tiff,
            },
        };
        state.ops.profiles.push(profile.clone());
        state.ops.profile = Some(0);
        let request = state.scan_submission().unwrap();
        assert_eq!(request.profile_id, Some(profile.id));
        assert_eq!(request.use_device_settings, None);
        assert_eq!(request.dpi, 300);
        assert_eq!(request.format, OriginalFormat::Tiff);
        assert!(request.duplex);
    }
    #[tokio::test]
    async fn lost_scan_response_retries_the_original_submission_without_duplicate_clicks() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (sent, mut received) = mpsc::channel(2);
        let server = tokio::spawn(async move {
            let (mut first, _) = listener.accept().await.unwrap();
            let request = read_request(&mut first).await;
            let scan: ScanRequest = serde_json::from_value(request["params"].clone()).unwrap();
            let record = ScanRecord::new(Device::default(), scan, "/fixture".into());
            sent.send(request).await.unwrap();
            drop(first); // Service accepted; response was lost.
            let (mut second, _) = listener.accept().await.unwrap();
            let request = read_request(&mut second).await;
            reply(&mut second, &request["id"], json!(record)).await;
            sent.send(request).await.unwrap();
            record.id
        });
        let mut state = State::default();
        state.client = Some(Client::new(&origin, &"a".repeat(64)).unwrap());
        state.ops.devices.push(Device {
            id: "fixture".into(),
            ..Device::default()
        });
        let notify = Arc::new(Notify::new());
        let wake = notify.clone();
        state
            .ops
            .mutation
            .set_waker(Arc::new(move || wake.notify_one()));
        state.start_scan();
        let original = state.ops.pending.as_ref().unwrap().params.clone();
        state.start_scan();
        assert_eq!(state.ops.pending.as_ref().unwrap().params, original);
        let failure = result(&mut state.ops.mutation, &notify).await;
        assert!(failure.is_err());
        state.finish_command(failure);
        assert!(state.ops.pending.is_some());
        state.ops.duplex = false; // Even changed controls cannot alter the accepted retry.
        state.retry_command();
        let success = result(&mut state.ops.mutation, &notify).await;
        state.finish_command(success);
        state.ops.watch.cancel();
        assert!(state.ops.pending.is_none());
        let first = received.recv().await.unwrap();
        let second = received.recv().await.unwrap();
        assert_eq!(first["params"], second["params"]);
        assert_eq!(first["params"]["dpi"], 600);
        assert_eq!(first["params"]["duplex"], true);
        assert_eq!(first["params"]["prepareCards"], true);
        assert_eq!(state.record.as_ref().unwrap().id, server.await.unwrap());
    }
}
