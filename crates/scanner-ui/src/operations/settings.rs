use super::Command;
use crate::{
    api,
    state::{Page, State},
    task::Task,
};
use rune::TextEdit;
use scanner_types::{
    error::{Result, ScannerError},
    scan::OriginalFormat,
    service::{DeviceStatus, ScanProfile, ScanSettings},
};
use serde_json::{Value, json};
use uuid::Uuid;

pub struct Settings {
    pub id: Option<Uuid>,
    pub name: TextEdit,
    pub dpi: TextEdit,
    pub duplex: bool,
    pub format: OriginalFormat,
    pub profile_offset: usize,
    pub inspection: Task<DeviceStatus>,
    pub report: Option<DeviceStatus>,
    pub notice: Option<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            id: None,
            name: TextEdit::new(""),
            dpi: TextEdit::new("600"),
            duplex: true,
            format: OriginalFormat::Png,
            profile_offset: 0,
            inspection: Task::default(),
            report: None,
            notice: None,
        }
    }
}
impl Settings {
    fn scan_settings(&self) -> Result<ScanSettings> {
        let dpi =
            self.dpi.text.trim().parse().map_err(|_| {
                ScannerError::new("invalidSettings", "Enter DPI between 150 and 600.")
            })?;
        let settings = ScanSettings {
            dpi,
            duplex: self.duplex,
            format: self.format,
        };
        settings.validate()?;
        Ok(settings)
    }
    pub(super) fn load(&mut self, profile: Option<&ScanProfile>) {
        self.id = profile.map(|p| p.id);
        self.name = TextEdit::new(profile.map(|p| p.name.as_str()).unwrap_or(""));
        self.dpi = TextEdit::new(profile.map(|p| p.settings.dpi).unwrap_or(600).to_string());
        self.duplex = profile.is_none_or(|p| p.settings.duplex);
        self.format = profile.map(|p| p.settings.format).unwrap_or_default();
        self.notice = None;
    }
}
impl State {
    pub fn show_settings(&mut self) {
        if self.fixture {
            return;
        }
        self.page = Page::Settings;
        self.discover();
    }
    pub fn new_profile(&mut self) {
        if !self.ops.busy() {
            self.ops.settings.load(None);
        }
    }
    pub fn edit_profile(&mut self, index: usize) {
        if self.ops.busy() {
            return;
        }
        if let Some(profile) = self.ops.profiles.get(index) {
            self.ops.settings.load(Some(profile));
        }
    }
    pub fn save_profile(&mut self) {
        let result = (|| -> Result<ScanProfile> {
            let profile = ScanProfile {
                id: self.ops.settings.id.unwrap_or_else(Uuid::new_v4),
                name: self.ops.settings.name.text.trim().into(),
                settings: self.ops.settings.scan_settings()?,
            };
            profile.validate()?;
            Ok(profile)
        })();
        match result {
            Ok(profile) => self.submit(Command {
                method: if self.ops.settings.id.is_some() {
                    "profiles.update"
                } else {
                    "profiles.create"
                },
                params: json!(profile),
                job: None,
                focus: false,
            }),
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    pub fn delete_profile(&mut self) {
        if let Some(id) = self.ops.settings.id {
            self.submit(Command {
                method: "profiles.delete",
                params: json!({"id":id}),
                job: None,
                focus: false,
            });
        }
    }
    pub fn save_device_defaults(&mut self) {
        let Some(device) = self.ops.devices.get(self.ops.device) else {
            return;
        };
        match self.ops.settings.scan_settings() {
            Ok(settings) => self.submit(Command { method: "settings.set", params: json!({"deviceID":device.id,"dpi":settings.dpi,"duplex":settings.duplex,"format":settings.format}), job: None, focus: false }),
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    pub fn inspect_device(&mut self) {
        let (Some(client), Some(device)) =
            (self.client.clone(), self.ops.devices.get(self.ops.device))
        else {
            return;
        };
        let id = device.id.clone();
        self.ops.settings.notice = None;
        self.ops
            .settings
            .inspection
            .request(async move { api::call(&client, "device.get", json!({"deviceID":id})).await });
    }
    pub(super) fn finish_settings(&mut self, command: &Command, value: &Value) {
        if command.method.starts_with("profiles.") {
            if command.method == "profiles.delete" {
                self.ops.settings.load(None);
                self.ops.settings.notice = Some("Profile removed.".into());
            } else if let Ok(profile) = serde_json::from_value::<ScanProfile>(value.clone()) {
                self.ops.settings.load(Some(&profile));
                self.ops.settings.notice = Some("Profile saved.".into());
            }
            self.discover();
        } else if command.method == "settings.set"
            && let Ok(report) = serde_json::from_value(value.clone())
        {
            self.ops.settings.report = Some(report);
            self.ops.settings.notice = Some(
                "Device defaults saved. Choose Device defaults when scanning to use them.".into(),
            );
        }
    }
    pub(crate) fn poll_settings(&mut self) -> bool {
        let Some(result) = self.ops.settings.inspection.take() else {
            return false;
        };
        match result {
            Ok(report) => self.ops.settings.report = Some(report),
            Err(error) => self.error = Some(error.to_string()),
        }
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_edits_preserve_identity_and_reject_invalid_scan_settings() {
        let profile = ScanProfile {
            id: Uuid::new_v4(),
            name: "Cards".into(),
            settings: ScanSettings {
                dpi: 300,
                duplex: false,
                format: OriginalFormat::Tiff,
            },
        };
        let mut settings = Settings::default();
        settings.load(Some(&profile));
        assert_eq!(settings.id, Some(profile.id));
        assert_eq!(settings.scan_settings().unwrap(), profile.settings);
        settings.dpi = TextEdit::new("601");
        assert!(settings.scan_settings().is_err());
        settings.dpi = TextEdit::new("NaN");
        assert!(settings.scan_settings().is_err());
    }
}
