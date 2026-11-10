use super::{
    manager::{Manager, error},
    phase::Phase,
    transfer_abi::{Event, Pending, UserInterface},
};
use scanner_types::error::{Result, ScannerError};
use std::ffi::c_void;

impl Manager {
    pub fn enable(&mut self) -> Result<()> {
        if self.source.is_none() || self.phase != Phase::Open {
            return Err(error("TWAIN source cannot be enabled in this state."));
        }
        let mut ui = UserInterface {
            show: 0,
            modal: 0,
            parent: self.parent,
        };
        self.source_call(1, 9, 0x502, (&mut ui as *mut UserInterface).cast())?;
        self.phase = Phase::Enabled;
        Ok(())
    }
    /// Forward a live Win32 MSG on its owning thread; true requests cancellation.
    pub unsafe fn process_event(&mut self, native: *mut c_void) -> Result<(bool, bool)> {
        let Some(source) = self.source.as_mut() else {
            return Ok((false, false));
        };
        if self.phase == Phase::Open {
            return Ok((false, false));
        }
        let mut event = Event { native, message: 0 };
        let status = unsafe {
            (self.entry)(
                &mut self.app,
                source,
                1,
                2,
                0x601,
                (&mut event as *mut Event).cast(),
            )
        };
        if !matches!(status, 4 | 5) {
            self.check(status, true)?;
            return Err(error("Unexpected TWAIN event return code."));
        }
        match event.message {
            0 => {}
            0x101 if self.phase == Phase::Enabled => self.phase = Phase::Ready,
            0x102 | 0x103 => return Ok((status == 4, true)),
            0x104 => {} // device events do not themselves authorize a transfer
            _ => return Err(error("Unexpected TWAIN source event or state.")),
        }
        Ok((status == 4, false))
    }
    pub fn end_transfer(&mut self) -> Result<()> {
        if self.phase != Phase::Transferring {
            return Err(error("No TWAIN transfer to end."));
        }
        let mut pending = Pending::default();
        self.source_call(1, 5, 0x701, (&mut pending as *mut Pending).cast())?;
        self.phase = if pending.count == 0 {
            Phase::Enabled
        } else {
            Phase::Ready
        };
        Ok(())
    }
    pub fn stop_transfer(&mut self) -> Result<()> {
        let result: Result<()> = (|| {
            if self.phase == Phase::Transferring {
                self.end_transfer()?;
            }
            if self.phase == Phase::Ready {
                let mut pending = Pending::default();
                self.source_call(1, 5, 7, (&mut pending as *mut Pending).cast())?;
                self.phase = Phase::Enabled;
            }
            if self.phase == Phase::Enabled {
                let mut ui = UserInterface {
                    show: 0,
                    modal: 0,
                    parent: self.parent,
                };
                self.source_call(1, 9, 0x501, (&mut ui as *mut UserInterface).cast())?;
                self.phase = Phase::Open;
            }
            Ok(())
        })();
        if let Err(failure) = result {
            self.quarantined = true;
            return Err(ScannerError::new(
                "cleanup",
                format!("Could not stop TWAIN acquisition: {failure}"),
            ));
        }
        Ok(())
    }
}
