use super::{
    connection::{Connection, Signals},
    delegate::BrowserDelegate,
};
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
use objc2_image_capture_core::{
    ICDeviceBrowser, ICDeviceLocationTypeMask, ICDeviceTypeMask, ICScannerDevice,
};
use scanner_types::{
    device::Device,
    error::{Result, ScannerError},
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct BrowserSignals {
    pub enumerated: Option<Instant>,
    pub connections: Vec<Weak<RefCell<Signals>>>,
}
pub(super) struct Controller {
    browser: Retained<ICDeviceBrowser>,
    _delegate: Retained<BrowserDelegate>,
    pub signals: Rc<RefCell<BrowserSignals>>,
    pub active: Rc<Cell<bool>>,
    pub quarantined: RefCell<Option<Connection>>,
    pub marker: MainThreadMarker,
    started: Cell<bool>,
    stopping: Cell<bool>,
}
pub(super) struct Ownership(Rc<Cell<bool>>);
impl Drop for Ownership {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
impl Controller {
    pub fn new(marker: MainThreadMarker) -> Self {
        let signals = Rc::new(RefCell::new(BrowserSignals::default()));
        let delegate = BrowserDelegate::new(marker, signals.clone());
        // SAFETY: browser/delegate live on the main thread; Controller retains the
        // weak delegate for at least the browser's lifetime.
        let browser = unsafe {
            let browser = ICDeviceBrowser::new();
            browser.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            browser.setBrowsedDeviceTypeMask(ICDeviceTypeMask(
                ICDeviceTypeMask::Scanner.0 | ICDeviceLocationTypeMask::Local.0,
            ));
            browser
        };
        Self {
            browser,
            _delegate: delegate,
            signals,
            active: Rc::new(Cell::new(false)),
            quarantined: RefCell::new(None),
            marker,
            started: Cell::new(false),
            stopping: Cell::new(false),
        }
    }
    pub async fn devices(&self) -> Result<Vec<Device>> {
        if self.stopping.get() {
            return Err(ScannerError::new(
                "busy",
                "Native scanner host is stopping.",
            ));
        }
        if !self.started.replace(true) {
            unsafe {
                self.browser.start();
            }
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let enumerated = self.signals.borrow().enumerated;
            if enumerated.is_some_and(|at| at.elapsed() >= Duration::from_millis(500)) {
                break;
            }
            if Instant::now() >= deadline {
                return Err(ScannerError::new(
                    "timeout",
                    "Local scanner discovery did not complete within 15 seconds.",
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let mut devices: Vec<_> = self.scanners().iter().map(|s| describe(s)).collect();
        devices.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(devices)
    }
    fn scanners(&self) -> Vec<Retained<ICScannerDevice>> {
        // SAFETY: main-thread access; the returned objects are retained by NSArray iteration.
        unsafe {
            self.browser
                .devices()
                .map(|devices| {
                    devices
                        .iter()
                        .filter_map(|d| d.downcast::<ICScannerDevice>().ok())
                        .collect()
                })
                .unwrap_or_default()
        }
    }
    pub fn acquire(&self) -> Result<Ownership> {
        if self.stopping.get() || self.active.get() || self.quarantined.borrow().is_some() {
            return Err(ScannerError::new(
                "busy",
                "Scanner session is active or requires disconnect recovery.",
            ));
        }
        self.active.set(true);
        Ok(Ownership(self.active.clone()))
    }
    pub fn connection(&self, id: &str) -> Result<Connection> {
        let scanner = self
            .scanners()
            .into_iter()
            .find(|s| describe(s).id == id)
            .ok_or_else(|| {
                ScannerError::new(
                    "deviceNotFound",
                    "Scanner disconnected before opening its session.",
                )
            })?;
        let connection = Connection::new(scanner, self.marker);
        self.signals
            .borrow_mut()
            .connections
            .push(Rc::downgrade(&connection.signals));
        Ok(connection)
    }
    pub fn stop(&self) {
        self.stopping.set(true);
        for weak in &self.signals.borrow().connections {
            if let Some(state) = weak.upgrade() {
                state
                    .borrow_mut()
                    .error
                    .get_or_insert_with(super::connection::cancelled);
            }
        }
    }
    pub fn recover(&self) -> bool {
        if self.active.get() {
            return false;
        }
        let removed = self
            .quarantined
            .borrow()
            .as_ref()
            .is_some_and(|c| c.signals.borrow().removed);
        if removed {
            self.quarantined.borrow_mut().take();
        }
        self.quarantined.borrow().is_none()
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        unsafe {
            self.browser.stop();
            self.browser.setDelegate(None);
        }
    }
}
pub(super) fn describe(scanner: &ICScannerDevice) -> Device {
    // SAFETY: only called by main-thread Controller/Connection operations.
    unsafe {
        Device {
            id: scanner
                .UUIDString()
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    format!(
                        "usb:{}:{}:{}",
                        scanner.usbVendorID(),
                        scanner.usbProductID(),
                        scanner.usbLocationID()
                    )
                }),
            name: scanner
                .name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Unnamed scanner".into()),
            transport: scanner.transportType().map(|s| s.to_string()),
            usb_vendor_id: scanner.usbVendorID(),
            usb_product_id: scanner.usbProductID(),
            driver_path: scanner.modulePath().to_string(),
            driver_module_version: scanner.moduleVersion().map(|s| s.to_string()),
            ..Device::default()
        }
    }
}
