use super::ocr_error;
use libloading::Library;
use scanner_types::error::{Result, ScannerError};
use std::{
    ffi::{c_int, c_void},
    ptr::NonNull,
    time::Instant,
};
type Handle = *mut c_void;
pub(super) struct Api {
    create: unsafe extern "C" fn() -> Handle,
    delete: unsafe extern "C" fn(Handle),
    set: unsafe extern "C" fn(Handle, c_int),
    recognize: unsafe extern "C" fn(Handle, Handle) -> c_int,
}
impl Api {
    pub(super) fn load(library: &Library) -> Result<Self> {
        // SAFETY: stable Tesseract C API signatures. The parent owns the library
        // until after this table and every monitor/engine have been dropped.
        unsafe {
            Ok(Self {
                create: *library.get(b"TessMonitorCreate\0").map_err(ocr_error)?,
                delete: *library.get(b"TessMonitorDelete\0").map_err(ocr_error)?,
                set: *library
                    .get(b"TessMonitorSetDeadlineMSecs\0")
                    .map_err(ocr_error)?,
                recognize: *library.get(b"TessBaseAPIRecognize\0").map_err(ocr_error)?,
            })
        }
    }
    pub(super) unsafe fn recognize(&self, engine: Handle, deadline: Instant) -> Result<()> {
        let millis = remaining(deadline)?;
        // SAFETY: engine is live on its owning worker; the monitor is private to
        // this call and stays alive throughout native recognition.
        let monitor = NonNull::new(unsafe { (self.create)() })
            .ok_or_else(|| ocr_error("Could not allocate OCR deadline monitor."))?;
        let monitor = Monitor {
            handle: monitor,
            api: self,
        };
        let result = unsafe {
            (self.set)(monitor.handle.as_ptr(), millis);
            (self.recognize)(engine, monitor.handle.as_ptr())
        };
        remaining(deadline)?;
        if result != 0 {
            return Err(ocr_error(
                "Tesseract recognition was interrupted or failed.",
            ));
        }
        Ok(())
    }
}
struct Monitor<'a> {
    handle: NonNull<c_void>,
    api: &'a Api,
}
impl Drop for Monitor<'_> {
    fn drop(&mut self) {
        // SAFETY: this is the sole owner, and the library outlives the monitor.
        unsafe {
            (self.api.delete)(self.handle.as_ptr());
        }
    }
}
pub(super) fn remaining(deadline: Instant) -> Result<c_int> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(ScannerError::new(
            "timeout",
            "Camera OCR deadline expired. Try a clearer view.",
        ));
    }
    Ok(left.as_millis().saturating_add(1).min(c_int::MAX as u128) as c_int)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };
    static CREATED: AtomicUsize = AtomicUsize::new(0);
    static DELETED: AtomicUsize = AtomicUsize::new(0);
    static CALLED: AtomicUsize = AtomicUsize::new(0);
    unsafe extern "C" fn create() -> Handle {
        CREATED.fetch_add(1, Ordering::SeqCst);
        Box::into_raw(Box::new(0_i32)).cast()
    }
    unsafe extern "C" fn delete(p: Handle) {
        DELETED.fetch_add(1, Ordering::SeqCst);
        // SAFETY: only pointers from create reach this fixture.
        drop(unsafe { Box::from_raw(p.cast::<i32>()) });
    }
    unsafe extern "C" fn set(p: Handle, millis: c_int) {
        // SAFETY: live monitor allocated by create.
        unsafe {
            *p.cast::<i32>() = millis;
        }
    }
    unsafe extern "C" fn fail(_: Handle, monitor: Handle) -> c_int {
        CALLED.fetch_add(1, Ordering::SeqCst);
        // SAFETY: live monitor allocated by create.
        let millis = unsafe { *monitor.cast::<i32>() };
        if (1..=10_001).contains(&millis) {
            -1
        } else {
            -2
        }
    }
    #[test]
    fn expired_operations_never_enter_native_ocr_and_failed_calls_free_monitors() {
        let api = Api {
            create,
            delete,
            set,
            recognize: fail,
        };
        // SAFETY: fixture recognize ignores the null engine pointer.
        let error = unsafe {
            api.recognize(
                std::ptr::null_mut(),
                Instant::now() - Duration::from_secs(1),
            )
        }
        .unwrap_err();
        assert_eq!(error.kind, "timeout");
        assert_eq!(CREATED.load(Ordering::SeqCst), 0);
        // SAFETY: fixture recognize ignores the null engine pointer.
        assert!(
            unsafe {
                api.recognize(
                    std::ptr::null_mut(),
                    Instant::now() + Duration::from_secs(10),
                )
            }
            .is_err()
        );
        assert_eq!(CREATED.load(Ordering::SeqCst), 1);
        assert_eq!(CALLED.load(Ordering::SeqCst), 1);
        assert_eq!(DELETED.load(Ordering::SeqCst), 1);
    }
}
