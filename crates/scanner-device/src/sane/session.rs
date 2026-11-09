use super::{
    acquisition,
    cancellation::Cancellation,
    configuration::Changes,
    ffi::{self, Api, Handle},
    options::Options,
};
use crate::{CaptureEvent, CaptureOutcome, persist_settings};
use scanner_types::{error::Result, scan::ScanRequest};
use std::{ffi::CString, path::Path, sync::Arc};
use tokio::{runtime::Handle as Runtime, sync::mpsc};

pub(super) struct Session<'a> {
    pub api: &'a Api,
    pub handle: Handle,
    cancellation: Option<Arc<Cancellation>>,
}
impl<'a> Session<'a> {
    pub fn open(api: &'a Api, id: &str) -> Result<Self> {
        let name = id
            .strip_prefix("sane:")
            .filter(|name| !name.is_empty() && name.len() <= 500)
            .ok_or_else(|| ffi::error("Invalid SANE device ID."))?;
        let name = CString::new(name).map_err(ffi::error)?;
        let mut handle = std::ptr::null_mut();
        // SAFETY: valid name and output pointer; all operations stay on this worker.
        unsafe {
            ffi::check((api.open)(name.as_ptr(), &mut handle), "open device")?;
        }
        if handle.is_null() {
            return Err(ffi::error("SANE returned an empty handle."));
        }
        Ok(Self {
            api,
            handle,
            cancellation: None,
        })
    }
    fn arm(&mut self, cancellation: Arc<Cancellation>) {
        cancellation.arm(self.api.cancel, self.handle);
        self.cancellation = Some(cancellation);
    }
    fn stop(&mut self) {
        if let Some(cancellation) = self.cancellation.take() {
            cancellation.disarm();
            // SAFETY: worker has returned from read/start. Disarm also waits for
            // an outstanding asynchronous cancel before restoring or closing.
            unsafe {
                (self.api.cancel)(self.handle);
            }
        }
    }
}
impl Drop for Session<'_> {
    fn drop(&mut self) {
        self.stop();
        // SAFETY: all reads returned; this is the sole owner of the open handle.
        unsafe {
            (self.api.close)(self.handle);
        }
    }
}
pub(super) struct Capture<'a> {
    pub id: &'a str,
    pub request: &'a ScanRequest,
    pub incoming: &'a Path,
    pub events: &'a mpsc::Sender<CaptureEvent>,
    pub cancellation: Arc<Cancellation>,
    pub runtime: &'a Runtime,
}
pub(super) fn capture(api: &Api, capture: Capture<'_>) -> CaptureOutcome {
    let mut session = match Session::open(api, capture.id) {
        Ok(session) => session,
        Err(error) => {
            return CaptureOutcome {
                errors: vec![error],
                cancelled: false,
                session_closed: true,
            };
        }
    };
    let mut changes = Changes::default();
    let result = (|| {
        if let Some(error) = capture.cancellation.failure() {
            return Err(error);
        }
        let options = Options {
            api,
            handle: session.handle,
        };
        changes.snapshot(&options)?;
        let settings = changes.configure(&options, capture.request)?;
        // Before issuing a physical start, reject an unsupported estimated frame.
        acquisition::parameters(&session)?;
        capture
            .runtime
            .block_on(persist_settings(capture.events, settings))?;
        session.arm(capture.cancellation.clone());
        acquisition::capture_pages(&session, &capture)
    })();
    session.stop();
    let mut errors: Vec<_> = result.err().into_iter().collect();
    errors.extend(changes.restore(&Options {
        api,
        handle: session.handle,
    }));
    drop(session);
    let cancelled = errors.iter().any(|error| error.kind == "cancelled");
    CaptureOutcome {
        errors,
        cancelled,
        session_closed: true,
    }
}
