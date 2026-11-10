use super::{Acknowledgement, Event, error};
use scanner_types::error::Result;
use std::{
    collections::HashMap,
    ffi::c_void,
    ptr,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use windows_sys::Win32::{
    System::Power::{
        DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, PowerRegisterSuspendResumeNotification,
        PowerUnregisterSuspendResumeNotification,
    },
    UI::WindowsAndMessaging::{
        DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
    },
};

pub(super) fn run(
    events: mpsc::UnboundedSender<Event>,
    stop: CancellationToken,
    ready: oneshot::Sender<Result<()>>,
) -> Result<()> {
    let context = register(events)?;
    let mut parameters = DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
        Callback: Some(callback),
        Context: context as *mut c_void,
    };
    let mut handle = ptr::null_mut();
    // SAFETY: parameters live through registration; Context is an opaque ID,
    // never a dereferenced pointer. The callback copies a registered sender.
    let code = unsafe {
        PowerRegisterSuspendResumeNotification(
            DEVICE_NOTIFY_CALLBACK,
            (&mut parameters as *mut DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS).cast(),
            &mut handle,
        )
    };
    if code != 0 {
        remove(context);
        let _ = ready.send(Err(error(format!(
            "Power notification registration failed ({code})."
        ))));
        return Ok(());
    }
    let _ = ready.send(Ok(()));
    while !stop.is_cancelled() {
        std::thread::park_timeout(Duration::from_millis(100));
    }
    // SAFETY: handle is owned from successful registration. Removing its ID
    // makes even an unexpectedly late callback harmless.
    let code = unsafe { PowerUnregisterSuspendResumeNotification(handle as isize) };
    remove(context);
    if code != 0 {
        return Err(error(format!(
            "Power notification cleanup failed ({code})."
        )));
    }

    Ok(())
}
unsafe extern "system" fn callback(context: *const c_void, kind: u32, _: *const c_void) -> u32 {
    let Some(sender) = registry()
        .lock()
        .ok()
        .and_then(|map| map.get(&(context as usize)).cloned())
    else {
        return 0;
    };
    let event = match kind {
        PBT_APMSUSPEND => Event::Suspend(Acknowledgement::new(|| {})),
        PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => Event::Resume,
        _ => return 0,
    };
    let _ = sender.send(event);
    0
}

fn registry() -> &'static Mutex<HashMap<usize, mpsc::UnboundedSender<Event>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<usize, mpsc::UnboundedSender<Event>>>> =
        OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}
fn register(sender: mpsc::UnboundedSender<Event>) -> Result<usize> {
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    let id = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .map_err(|_| error("Power registration IDs exhausted."))?;
    registry()
        .lock()
        .map_err(|_| error("Power registry lock failed."))?
        .insert(id, sender);
    Ok(id)
}
fn remove(id: usize) {
    if let Ok(mut map) = registry().lock() {
        map.remove(&id);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_routes_events_and_late_unregistered_callbacks_are_harmless() {
        let (sender, mut receiver) = mpsc::unbounded_channel::<Event>();
        let id = register(sender).unwrap();
        // SAFETY: callback treats context as an opaque registered ID.
        unsafe {
            callback(id as *const c_void, PBT_APMSUSPEND, ptr::null());
            callback(id as *const c_void, PBT_APMRESUMEAUTOMATIC, ptr::null());
            callback(id as *const c_void, 0xffff, ptr::null());
        }
        assert!(matches!(receiver.try_recv(), Ok(Event::Suspend(_))));
        assert!(matches!(receiver.try_recv(), Ok(Event::Resume)));
        remove(id);
        // SAFETY: an unknown ID is ignored without dereferencing it.
        unsafe {
            callback(id as *const c_void, PBT_APMSUSPEND, ptr::null());
        }
        assert!(receiver.try_recv().is_err());
    }
}
