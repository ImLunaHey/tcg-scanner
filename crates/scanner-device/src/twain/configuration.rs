use super::{
    capabilities,
    container::Scalar,
    manager::{Manager, error},
};
use scanner_types::{
    Extensions,
    error::{Result, ScannerError},
    scan::ScanRequest,
};

const CONTROLS: &[(u16, u16, u32, bool)] = &[
    (0x1002, 6, 1, true),
    (0x1007, 6, 1, false),
    (0x1013, 6, 0, false),
    (0x0101, 4, 2, true),
    (0x112b, 4, 8, true),
    (0x0102, 4, 0, true),
    (0x1122, 4, 1, true),
    (0x1118, 7, 600, true),
    (0x1119, 7, 600, true),
    (0x0103, 4, 2, true),
    (0x0001, 1, 65535, true),
    (0x1134, 2, (-2i32) as u32, false),
    (0x1150, 6, 0, false),
    (0x1151, 6, 0, false),
    (0x1152, 6, 0, false),
    (0x1149, 4, 0, false),
];
#[derive(Default)]
pub(super) struct Changes {
    originals: Vec<(u16, Scalar)>,
    changed: bool,
}
impl Changes {
    pub fn configure(
        &mut self,
        manager: &mut Manager,
        request: &ScanRequest,
    ) -> Result<Extensions> {
        request.validate()?;
        if request.background.is_some() {
            return Err(ScannerError::new(
                "unsupported",
                "TWAIN has no portable backing-colour control; a validated driver-specific mapping is required.",
            ));
        }
        for &(id, _, _, _) in CONTROLS {
            match capabilities::read(manager, id, 2) {
                Ok(value) => self.originals.push((id, value)),
                Err(failure) if failure.kind == "unsupported" => {}
                Err(failure) => return Err(failure),
            }
        }
        let mut desired = Vec::new();
        for &(id, kind, raw, required) in CONTROLS {
            let (raw, required) = match id {
                0x1013 => (u32::from(request.duplex), request.duplex),
                0x1118 | 0x1119 => (request.dpi, true),
                0x1149 => match request.overscan {
                    Some(value) => (if value { 4 } else { 0 }, true),
                    None => continue,
                },
                _ => (raw, required),
            };
            let old = match capabilities::read(manager, id, 2) {
                Ok(value) => value,
                Err(failure) if !required && failure.kind == "unsupported" => continue,
                Err(failure) => return Err(failure),
            };
            if old.kind != kind {
                return Err(error(format!(
                    "Unexpected TWAIN type for capability {id:#06x}."
                )));
            }
            if !self.originals.iter().any(|(key, _)| *key == id) {
                self.originals.push((id, old));
            }
            let value = Scalar { kind, raw };
            if old != value {
                self.changed = true;
                capabilities::write(manager, id, value)?;
            }
            desired.push((id, value));
        }
        let mut settings = Extensions::new();
        settings.insert("backend".into(), "twain".into());
        for (id, value) in desired {
            let actual = capabilities::read(manager, id, 2)?;
            if actual != value {
                return Err(error(format!("TWAIN did not apply capability {id:#06x}.")));
            }
            settings.insert(format!("capability-{id:#06x}"), actual.json());
        }
        let loaded = capabilities::read(manager, 0x1003, 2)?;
        if loaded.kind != 6 || loaded.raw != 1 {
            return Err(ScannerError::new(
                "feederEmpty",
                "TWAIN feeder is not loaded.",
            ));
        }
        Ok(settings)
    }
    pub fn restore(&self, manager: &mut Manager) -> Vec<ScannerError> {
        if !self.changed {
            return Vec::new();
        }
        let mut errors = Vec::new();
        for &(id, original) in &self.originals {
            let result = (|| {
                if capabilities::read(manager, id, 2)? != original {
                    capabilities::write(manager, id, original)?;
                }
                if capabilities::read(manager, id, 2)? != original {
                    return Err(error("TWAIN restoration readback differed."));
                }
                Ok(())
            })();
            if let Err(error) = result {
                errors.push(ScannerError::new(
                    "cleanup",
                    format!("Could not restore TWAIN {id:#06x}: {error}"),
                ));
            }
        }
        errors
    }
}
