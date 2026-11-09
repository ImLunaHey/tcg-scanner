use super::ffi::{Cancel, Handle};
use scanner_types::error::ScannerError;
use std::{
    sync::{
        Mutex,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};

struct Active {
    cancel: Cancel,
    handle: usize,
    started: Instant,
    last_frame: Instant,
}
#[derive(Default)]
pub(super) struct Cancellation {
    active: Mutex<Option<Active>>,
    reason: AtomicU8,
}
impl Cancellation {
    pub fn arm(&self, cancel: Cancel, handle: Handle) {
        let now = Instant::now();
        *self.active.lock().unwrap() = Some(Active {
            cancel,
            handle: handle as usize,
            started: now,
            last_frame: now,
        });
    }
    pub fn disarm(&self) {
        self.active.lock().unwrap().take();
    }
    pub fn received_frame(&self) {
        if let Some(active) = self.active.lock().unwrap().as_mut() {
            active.last_frame = Instant::now();
        }
    }
    pub fn poll(&self, user_cancelled: bool) {
        let active = self.active.lock().unwrap();
        if user_cancelled {
            self.reason
                .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
                .ok();
        }
        if let Some(active) = active.as_ref() {
            if active.started.elapsed() >= Duration::from_secs(1800)
                || active.last_frame.elapsed() >= Duration::from_secs(120)
            {
                self.reason
                    .compare_exchange(0, 2, Ordering::SeqCst, Ordering::SeqCst)
                    .ok();
            }
            if self.reason.load(Ordering::SeqCst) != 0 {
                // SAFETY: SANE 1 explicitly permits asynchronous sane_cancel.
                // This is the sole cross-thread native operation. Holding this
                // lock prevents disarm/close/unload while the handle is in use.
                unsafe {
                    (active.cancel)(active.handle as Handle);
                }
            }
        }
    }
    pub fn failure(&self) -> Option<ScannerError> {
        match self.reason.load(Ordering::SeqCst) {
            1 => Some(ScannerError::new("cancelled", "Operation cancelled.")),
            2 => Some(ScannerError::new(
                "timeout",
                "SANE capture exceeded its progress or batch deadline.",
            )),
            _ => None,
        }
    }
}
