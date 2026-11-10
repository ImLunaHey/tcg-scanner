use super::abi::{self, Entry, EntryPoints, Identity, Status};
use super::phase::Phase;
use libloading::Library;
use scanner_types::{
    device::Device,
    error::{Result, ScannerError},
};
use std::{
    ffi::c_void,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
pub(super) struct Memory {
    pub allocate: abi::Allocate,
    pub free: abi::Free,
    pub lock: abi::Lock,
    pub unlock: abi::Unlock,
}
pub(super) struct Manager {
    library: Option<Library>,
    pub app: Identity,
    pub source: Option<Identity>,
    pub entry: Entry,
    pub memory: Option<Memory>,
    pub parent: *mut c_void,
    pub phase: Phase,
    opened: bool,
    pub quarantined: bool,
    path: PathBuf,
}
impl Manager {
    /// Parent must remain a valid window owned by this thread through shutdown.
    pub unsafe fn load(path: &Path, parent: *mut c_void) -> Result<Self> {
        // SAFETY: application-selected trusted DSM; function follows twain.h.
        let library = unsafe { Library::new(path) }.map_err(error)?;
        let entry = unsafe { *library.get::<Entry>(b"DSM_Entry\0").map_err(error)? };
        Ok(Self {
            library: Some(library),
            app: Identity::application(),
            source: None,
            entry,
            memory: None,
            parent,
            phase: Phase::Open,
            opened: false,
            quarantined: false,
            path: path.into(),
        })
    }
    pub fn initialize(&mut self) -> Result<()> {
        let mut parent = self.parent;
        self.manager_call(4, 0x301, (&mut parent as *mut *mut c_void).cast())?;
        self.opened = true;
        let mut functions = EntryPoints {
            size: std::mem::size_of::<EntryPoints>() as u32,
            entry: None,
            allocate: None,
            free: None,
            lock: None,
            unlock: None,
        };
        self.manager_call(0x403, 1, (&mut functions as *mut EntryPoints).cast())?;
        self.memory = Some(Memory {
            allocate: functions
                .allocate
                .ok_or_else(|| error("DSM allocator missing."))?,
            free: functions.free.ok_or_else(|| error("DSM free missing."))?,
            lock: functions.lock.ok_or_else(|| error("DSM lock missing."))?,
            unlock: functions
                .unlock
                .ok_or_else(|| error("DSM unlock missing."))?,
        });
        Ok(())
    }
    pub fn manager_call(&mut self, dat: u16, message: u16, data: *mut c_void) -> Result<()> {
        // SAFETY: caller supplies the struct required by the triplet. Only the
        // owner thread invokes entry points; application storage remains live.
        let status =
            unsafe { (self.entry)(&mut self.app, std::ptr::null_mut(), 1, dat, message, data) };
        self.check(status, false)
    }
    pub fn source_call(
        &mut self,
        group: u32,
        dat: u16,
        message: u16,
        data: *mut c_void,
    ) -> Result<()> {
        let source = self
            .source
            .as_mut()
            .ok_or_else(|| error("No TWAIN source is open."))?;
        let status = unsafe { (self.entry)(&mut self.app, source, group, dat, message, data) };
        self.check(status, true)
    }
    pub fn check(&mut self, status: u16, source: bool) -> Result<()> {
        if status == 0 {
            return Ok(());
        }
        let mut detail = Status::default();
        let destination = if source {
            self.source
                .as_mut()
                .map_or(std::ptr::null_mut(), |source| source as *mut Identity)
        } else {
            std::ptr::null_mut()
        };
        // SAFETY: DAT_STATUS is valid after failure and copies into Status.
        let read = unsafe {
            (self.entry)(
                &mut self.app,
                destination,
                1,
                8,
                1,
                (&mut detail as *mut Status).cast(),
            )
        };
        let condition = if read == 0 {
            Some(detail.condition)
        } else {
            None
        };
        let kind = match condition {
            Some(2) => "resourceLimit",
            Some(3 | 23) => "deviceNotFound",
            Some(6 | 13 | 14) => "unsupported",
            Some(16) => "accessDenied",
            Some(20) => "paperJam",
            Some(21) => "doubleFeed",
            Some(29) => "feederEmpty",
            _ => {
                if status == 3 {
                    "cancelled"
                } else {
                    "driver"
                }
            }
        };
        Err(ScannerError {
            kind: kind.into(),
            message: format!("TWAIN operation failed (return {status}, condition {condition:?})."),
            domain: Some("TWAIN".into()),
            code: condition.map(i64::from),
        })
    }
    pub fn identities(&mut self) -> Result<Vec<Identity>> {
        self.ensure_available()?;
        if self.source.is_some() {
            return Err(error("Cannot enumerate during a TWAIN session."));
        }
        let mut identities = Vec::new();
        let mut message = 4;
        loop {
            let mut identity = Identity::empty();
            let status = unsafe {
                (self.entry)(
                    &mut self.app,
                    std::ptr::null_mut(),
                    1,
                    3,
                    message,
                    (&mut identity as *mut Identity).cast(),
                )
            };
            if status == 7 {
                return Ok(identities);
            }
            self.check(status, false)?;
            if identities.len() == 64 {
                return Err(error("Too many TWAIN sources."));
            }
            identities.push(identity);
            message = 5;
        }
    }
    pub fn device(&self, identity: Identity) -> Device {
        let version = identity.version;
        Device {
            id: device_id(identity),
            name: abi::string(identity.product),
            driver_path: self.path.display().to_string(),
            driver_module_version: Some(format!("{}.{}", { version.major }, { version.minor })),
            extensions: [
                (
                    "twainManufacturer".into(),
                    abi::string(identity.manufacturer).into(),
                ),
                (
                    "twainProductFamily".into(),
                    abi::string(identity.family).into(),
                ),
            ]
            .into(),
            ..Device::default()
        }
    }
    pub fn open_source(&mut self, id: &str) -> Result<Device> {
        let mut identity = self
            .identities()?
            .into_iter()
            .find(|identity| device_id(*identity) == id)
            .ok_or_else(|| ScannerError::new("deviceNotFound", "TWAIN source not found."))?;
        self.manager_call(3, 0x401, (&mut identity as *mut Identity).cast())?;
        self.source = Some(identity);
        Ok(self.device(identity))
    }
    pub fn close_source(&mut self) -> Result<()> {
        if self.phase != Phase::Open {
            return Err(error("TWAIN acquisition must stop before source closure."));
        }
        if let Some(mut identity) = self.source {
            if let Err(error) = self.manager_call(3, 0x402, (&mut identity as *mut Identity).cast())
            {
                self.quarantined = true;
                return Err(ScannerError::new("closeSession", error.to_string()));
            }
            self.source = None;
        }
        Ok(())
    }
    pub fn ensure_available(&self) -> Result<()> {
        if self.quarantined {
            Err(ScannerError::new(
                "busy",
                "TWAIN session closure is uncertain; restart the service.",
            ))
        } else {
            Ok(())
        }
    }
}
impl Manager {
    pub fn shutdown(&mut self) -> Result<()> {
        self.ensure_available()?;
        self.stop_transfer()?;
        self.close_source()?;
        if self.opened {
            let mut parent = self.parent;
            if let Err(error) =
                self.manager_call(4, 0x302, (&mut parent as *mut *mut c_void).cast())
            {
                self.quarantined = true;
                return Err(error);
            }
            self.opened = false;
        }
        Ok(())
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        if self.shutdown().is_err() {
            // Worker keeps its thread/window alive on uncertain shutdown. This
            // final guard also prevents unloading the DSM during unwinding.
            if let Some(library) = self.library.take() {
                std::mem::forget(library);
            }
        }
    }
}
pub(super) fn error(error: impl std::fmt::Display) -> ScannerError {
    ScannerError::new("driver", error.to_string())
}
fn device_id(identity: Identity) -> String {
    // DSM IDs can change across enumeration. Preserve the driver identity text
    // as hex components, avoiding ambiguous separators and locale conversion.
    let mut id = String::from("twain:");
    for field in [identity.manufacturer, identity.family, identity.product] {
        for byte in field.into_iter().take_while(|byte| *byte != 0) {
            use std::fmt::Write;
            write!(&mut id, "{byte:02x}").unwrap();
        }
        id.push(':');
    }
    id
}

#[cfg(test)]
pub(super) static FIXTURE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_dsm_inspection_frees_containers_and_quarantines_failed_close() {
        let _guard = super::FIXTURE_LOCK.lock().unwrap();
        let Some(path) = std::env::var_os("SCANNER_TWAIN_TEST_LIBRARY") else {
            return;
        };
        let path = PathBuf::from(path);
        // SAFETY: explicitly selected test-only fixture with known ABI exports;
        // unlike real Windows DSMs it does not require a window.
        unsafe {
            let fixture = Library::new(&path).unwrap();
            let configure = fixture
                .get::<unsafe extern "C" fn(i32)>(b"fixture_configure\0")
                .unwrap();
            let counter = fixture
                .get::<unsafe extern "C" fn(i32) -> i32>(b"fixture_counter\0")
                .unwrap();
            configure(0);
            let mut manager = Manager::load(&path, std::ptr::null_mut()).unwrap();
            manager.initialize().unwrap();
            let identity = manager.identities().unwrap().remove(0);
            let device = manager.device(identity);
            assert_eq!(device.name, "Cards");
            let inspection = super::super::inspection::inspect(&mut manager, &device.id).unwrap();
            assert!(inspection.session_opened && inspection.session_closed);
            assert!(inspection.errors.is_empty());
            assert_eq!(inspection.units[0].settings["xResolution"].current, 300.0);
            assert_eq!(
                inspection.units[0].settings["xResolution"].allowed,
                Some(serde_json::json!({"minimum":150.0,"maximum":600.0,"step":150.0}))
            );
            assert!(!inspection.units[0].settings["xResolution"].read_only);
            assert!(inspection.units[0].settings["feederLoaded"].read_only);
            assert_eq!(counter(1), 0);
            assert_eq!(counter(2), 0);
            assert_eq!(counter(3), 0);
            manager.shutdown().unwrap();
            assert_eq!(counter(0), 0);
            drop(manager);
            configure(1);
            let mut manager = Manager::load(&path, std::ptr::null_mut()).unwrap();
            manager.initialize().unwrap();
            let inspection = super::super::inspection::inspect(&mut manager, &device.id).unwrap();
            assert!(!inspection.session_closed);
            assert!(manager.quarantined);
            assert!(manager.identities().is_err());
            assert!(manager.shutdown().is_err());
            assert_eq!(counter(4), 0);
            assert_eq!(counter(2), 0);
            assert_eq!(counter(3), 0);
            // A real worker retains its thread, parent window and manager on
            // this path. Simulate that lifetime without opening native hardware.
            std::mem::forget(manager);
        }
    }
}
