use super::delegate::SessionDelegate;
use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
use objc2_image_capture_core::{ICDevice, ICScannerDevice, ICScannerFunctionalUnitType};
use scanner_types::error::{Result, ScannerError};
use std::{
    cell::{Cell, RefCell},
    collections::{HashSet, VecDeque},
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

pub(super) struct Signals {
    pub identity: usize,
    pub open_requested: bool,
    pub open_completed: bool,
    pub opened: bool,
    pub ready: bool,
    pub selected: bool,
    pub closing: bool,
    pub close_completed: bool,
    pub closed: bool,
    pub removed: bool,
    pub error: Option<ScannerError>,
    pub scan_requested: bool,
    pub button_armed: bool,
    pub button_accepted: bool,
    pub scan_completed: bool,
    pub files: VecDeque<(u32, PathBuf)>,
    pub total: u32,
    pub last_image: Instant,
    seen: HashSet<PathBuf>,
}
impl Signals {
    fn new(identity: usize) -> Self {
        Self {
            identity,
            open_requested: false,
            open_completed: false,
            opened: false,
            ready: false,
            selected: false,
            closing: false,
            close_completed: false,
            closed: false,
            removed: false,
            error: None,
            scan_requested: false,
            button_armed: false,
            button_accepted: false,
            scan_completed: false,
            files: VecDeque::new(),
            total: 0,
            last_image: Instant::now(),
            seen: HashSet::new(),
        }
    }
    pub fn remove(&mut self, id: usize) {
        if self.identity == id {
            self.removed = true;
            self.button_armed = false;
            self.error = Some(ScannerError::new(
                "disconnected",
                "Scanner disconnected; received originals are retained.",
            ));
        }
    }
    pub fn receive(&mut self, path: Option<PathBuf>) -> bool {
        if self.total >= 100 {
            self.error = Some(ScannerError::new(
                "resourceLimit",
                "Capture limit reached; staging retained.",
            ));
            return true;
        }
        let Some(path) = path else {
            self.error = Some(ScannerError::new(
                "driver",
                "Driver did not provide a file URL.",
            ));
            return true;
        };
        if !self.scan_requested || !self.seen.insert(path.clone()) {
            self.error = Some(ScannerError::new(
                "driver",
                "Unexpected or repeated output URL; staging retained.",
            ));
            return true;
        }
        self.total += 1;
        self.last_image = Instant::now();
        self.files.push_back((self.total, path));
        if self.total == 100 {
            self.error = Some(ScannerError::new(
                "resourceLimit",
                "Basic scan limit of 100 images reached; cancelling.",
            ));
            true
        } else {
            false
        }
    }
}
pub(super) struct Connection {
    pub scanner: Retained<ICScannerDevice>,
    _delegate: Retained<SessionDelegate>,
    pub signals: Rc<RefCell<Signals>>,
    original_unit: Cell<Option<ICScannerFunctionalUnitType>>,
}
impl Connection {
    pub fn new(scanner: Retained<ICScannerDevice>, marker: MainThreadMarker) -> Self {
        let signals = Rc::new(RefCell::new(Signals::new(identity(&scanner))));
        let delegate = SessionDelegate::new(marker, signals.clone());
        // SAFETY: all objects stay on the main thread; the weak delegate is retained here.
        unsafe {
            scanner.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        }
        Self {
            scanner,
            _delegate: delegate,
            signals,
            original_unit: Cell::new(None),
        }
    }
    pub async fn connect(&self, cancel: &CancellationToken) -> Result<()> {
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        self.signals.borrow_mut().open_requested = true;
        unsafe {
            self.scanner.requestOpenSession();
        }
        self.wait("open session", cancel, |s| s.open_completed)
            .await?;
        self.wait("device readiness", cancel, |s| s.ready).await?;
        self.original_unit.set(Some(unsafe {
            self.scanner.selectedFunctionalUnit().r#type()
        }));
        Ok(())
    }
    pub async fn select(
        &self,
        unit: ICScannerFunctionalUnitType,
        cancel: &CancellationToken,
    ) -> Result<()> {
        if unsafe { self.scanner.selectedFunctionalUnit().r#type() } == unit {
            return Ok(());
        }
        self.signals.borrow_mut().selected = false;
        unsafe {
            self.scanner.requestSelectFunctionalUnit(unit);
        }
        self.wait("select functional unit", cancel, |s| s.selected)
            .await?;
        if unsafe { self.scanner.selectedFunctionalUnit().r#type() } != unit {
            return Err(ScannerError::new(
                "driver",
                "Unexpected functional unit selected.",
            ));
        }
        Ok(())
    }
    async fn wait(
        &self,
        operation: &str,
        cancel: &CancellationToken,
        done: impl Fn(&Signals) -> bool,
    ) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if cancel.is_cancelled() {
                return Err(cancelled());
            }
            {
                let state = self.signals.borrow();
                if let Some(error) = &state.error {
                    return Err(error.clone());
                }
                if done(&state) {
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                return Err(ScannerError::new(
                    "timeout",
                    format!("Timed out waiting for {operation}."),
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    pub async fn close(&self) -> Vec<ScannerError> {
        let mut errors = vec![];
        let restore = {
            let s = self.signals.borrow();
            !s.removed && s.opened && !s.close_completed && s.error.is_none()
        };
        if restore
            && let Some(unit) = self.original_unit.get()
            && let Err(error) = self.select(unit, &CancellationToken::new()).await
        {
            errors.push(error);
        }
        let should_close = {
            let s = self.signals.borrow();
            !s.removed && !s.close_completed && s.open_requested && (s.opened || !s.open_completed)
        };
        if should_close {
            self.signals.borrow_mut().closing = true;
            unsafe {
                self.scanner.requestCloseSession();
            }
            let deadline = Instant::now() + Duration::from_secs(10);
            while !self.signals.borrow().removed
                && !self.signals.borrow().close_completed
                && Instant::now() < deadline
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            let s = self.signals.borrow();
            if !s.removed && !s.close_completed {
                errors.push(ScannerError::new(
                    "cleanup",
                    "Scanner session closure was not confirmed.",
                ));
            }
        }
        if let Some(error) = self.signals.borrow().error.clone() {
            errors.push(error);
        }
        unsafe {
            self.scanner.setDelegate(None);
        }
        errors
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        // Last-resort cleanup if the host is torn down before its normal drain.
        // This does not assert successful closure; ordinary operations await callbacks.
        let (cancel, close) = {
            let s = self.signals.borrow();
            (
                !s.removed && s.scan_requested && !s.scan_completed,
                !s.removed && s.open_requested && !s.closed,
            )
        };
        unsafe {
            if cancel {
                self.scanner.cancelScan();
            }
            if close {
                self.scanner.requestCloseSession();
            }
            self.scanner.setDelegate(None);
        }
    }
}
pub(super) fn identity(device: &ICDevice) -> usize {
    device as *const ICDevice as usize
}
pub(super) fn cancelled() -> ScannerError {
    ScannerError::new("cancelled", "Operation cancelled.")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_exact_device_removal_releases_ownership_evidence() {
        let mut state = Signals::new(42);
        state.remove(43);
        assert!(!state.removed);
        state.remove(42);
        assert!(state.removed);
    }
    #[test]
    fn output_is_ordered_bounded_and_never_accepts_reused_urls() {
        let mut state = Signals::new(42);
        state.scan_requested = true;
        assert!(!state.receive(Some("/incoming/first.png".into())));
        assert_eq!(state.files[0].0, 1);
        assert!(state.receive(Some("/incoming/first.png".into())));
        assert_eq!(state.files.len(), 1);
        let mut state = Signals::new(1);
        state.scan_requested = true;
        for i in 0..100 {
            assert_eq!(
                state.receive(Some(format!("/incoming/{i}.png").into())),
                i == 99
            );
        }
        assert!(state.receive(Some("/incoming/extra.png".into())));
        assert_eq!(state.files.len(), 100);
    }
}
