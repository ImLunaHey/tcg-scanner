use super::{Acknowledgement, Event, error};
use scanner_types::error::Result;
use std::{cell::Cell, ffi::c_void, ptr, sync::mpsc as sync};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

type Reference = *mut c_void;
struct Context {
    connection: Cell<u32>,
    events: mpsc::UnboundedSender<Event>,
    acknowledge: sync::Sender<isize>,
}
struct Registration {
    context: Box<Context>,
    port: Reference,
    notifier: u32,
    source: Reference,
}
impl Registration {
    fn new(events: mpsc::UnboundedSender<Event>, acknowledge: sync::Sender<isize>) -> Result<Self> {
        let mut context = Box::new(Context {
            connection: Cell::new(0),
            events,
            acknowledge,
        });
        let mut port = ptr::null_mut();
        let mut notifier = 0;
        // SAFETY: the boxed context remains stable until deregistration on this
        // same run-loop thread. Outputs are initialized by successful registration.
        let connection = unsafe {
            IORegisterForSystemPower(
                (&mut *context as *mut Context).cast(),
                &mut port,
                callback,
                &mut notifier,
            )
        };
        if connection == 0 {
            return Err(error("IORegisterForSystemPower failed."));
        }
        context.connection.set(connection);
        // SAFETY: port was returned by successful IOKit registration.
        let source = unsafe { IONotificationPortGetRunLoopSource(port) };
        let registration = Self {
            context,
            port,
            notifier,
            source,
        };
        if source.is_null() {
            return Err(error("Power notification run-loop source is missing."));
        }
        // SAFETY: source and the current run loop are alive through registration.
        unsafe {
            CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopDefaultMode);
        }
        Ok(registration)
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        // SAFETY: removal and deregistration run on the callback's own thread,
        // before its context is freed. Each native owner is released exactly once.
        unsafe {
            IODeregisterForSystemPower(&mut self.notifier);
            if !self.source.is_null() {
                CFRunLoopRemoveSource(CFRunLoopGetCurrent(), self.source, kCFRunLoopDefaultMode);
            }
            IOServiceClose(self.context.connection.get());
            IONotificationPortDestroy(self.port);
        }
    }
}
pub(super) fn run(
    events: mpsc::UnboundedSender<Event>,
    stop: CancellationToken,
    ready: oneshot::Sender<Result<()>>,
) -> Result<()> {
    let (acknowledge, acknowledgements) = sync::channel();
    let registration = match Registration::new(events, acknowledge) {
        Ok(value) => value,
        Err(error) => {
            let _ = ready.send(Err(error));
            return Ok(());
        }
    };
    let _ = ready.send(Ok(()));
    while !stop.is_cancelled() {
        // SAFETY: this worker owns the registered run-loop source and context.
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, 1);
        }
        for notification in acknowledgements.try_iter() {
            // SAFETY: the notification came from this registration; connection is live.
            unsafe {
                IOAllowPowerChange(registration.context.connection.get(), notification);
            }
        }
    }
    Ok(())
}
unsafe extern "C" fn callback(context: Reference, _: u32, kind: u32, argument: Reference) {
    // SAFETY: IOKit invokes on the owning run-loop thread while context is live.
    let context = unsafe { &*context.cast::<Context>() };
    match kind {
        0xe0000270 => {
            // SAFETY: acknowledge this registration's CanSystemSleep message.
            unsafe {
                IOAllowPowerChange(context.connection.get(), argument as isize);
            }
        }
        0xe0000280 => {
            let sender = context.acknowledge.clone();
            let notification = argument as isize;
            let ack = Acknowledgement::new(move || {
                let _ = sender.send(notification);
            });
            let _ = context.events.send(Event::Suspend(ack));
        }
        0xe0000300 => {
            let _ = context.events.send(Event::Resume);
        }
        _ => {}
    }
}
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IORegisterForSystemPower(
        context: Reference,
        port: *mut Reference,
        callback: unsafe extern "C" fn(Reference, u32, u32, Reference),
        notifier: *mut u32,
    ) -> u32;
    fn IODeregisterForSystemPower(notifier: *mut u32) -> i32;
    fn IOAllowPowerChange(connection: u32, notification: isize) -> i32;
    fn IOServiceClose(connection: u32) -> i32;
    fn IONotificationPortGetRunLoopSource(port: Reference) -> Reference;
    fn IONotificationPortDestroy(port: Reference);
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopDefaultMode: Reference;
    fn CFRunLoopGetCurrent() -> Reference;
    fn CFRunLoopAddSource(run_loop: Reference, source: Reference, mode: Reference);
    fn CFRunLoopRemoveSource(run_loop: Reference, source: Reference, mode: Reference);
    fn CFRunLoopRunInMode(mode: Reference, seconds: f64, return_after_source: u8) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sleep_ack_is_deferred_and_wake_is_ordered_after_sleep() {
        let (events, mut received) = mpsc::unbounded_channel();
        let (acknowledge, acknowledged) = sync::channel();
        let mut context = Context {
            connection: Cell::new(0),
            events,
            acknowledge,
        };
        // SAFETY: these message kinds use only the live test context, not IOKit.
        unsafe {
            callback(
                (&mut context as *mut Context).cast(),
                0,
                0xe0000280,
                123isize as Reference,
            );
            callback(
                (&mut context as *mut Context).cast(),
                0,
                0xe0000300,
                ptr::null_mut(),
            );
        }
        assert!(acknowledged.try_recv().is_err());
        let Event::Suspend(ack) = received.try_recv().unwrap() else {
            panic!("Expected sleep");
        };
        assert!(matches!(received.try_recv(), Ok(Event::Resume)));
        drop(ack);
        assert_eq!(acknowledged.try_recv().unwrap(), 123);
    }
}
