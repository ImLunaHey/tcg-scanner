use super::{
    browser::BrowserSignals,
    connection::{Signals, identity},
};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_foundation::{NSError, NSObject, NSObjectProtocol, NSURL};
use objc2_image_capture_core::{
    ICDevice, ICDeviceBrowser, ICDeviceBrowserDelegate, ICDeviceDelegate, ICScannerDevice,
    ICScannerDeviceDelegate, ICScannerFunctionalUnit,
};
use scanner_types::error::ScannerError;
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Instant};

define_class!(
    // SAFETY: NSObject has no extra subclass invariants; all callbacks are main-thread only.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "ScannerRustBrowserDelegate"]
    #[ivars = Rc<RefCell<BrowserSignals>>]
    pub(super) struct BrowserDelegate;
    unsafe impl NSObjectProtocol for BrowserDelegate {}
    unsafe impl ICDeviceBrowserDelegate for BrowserDelegate {
        #[unsafe(method(deviceBrowser:didAddDevice:moreComing:))]
        fn added(&self,_browser:&ICDeviceBrowser,_device:&ICDevice,_more:bool) {}
        #[unsafe(method(deviceBrowser:didRemoveDevice:moreGoing:))]
        fn removed(&self,_browser:&ICDeviceBrowser,device:&ICDevice,_more:bool) {
            let mut state=self.ivars().borrow_mut();
            state.connections.retain(|weak|{
                if let Some(signal)=weak.upgrade() {signal.borrow_mut().remove(identity(device));true} else {false}
            });
        }
        #[unsafe(method(deviceBrowserDidEnumerateLocalDevices:))]
        fn enumerated(&self,_browser:&ICDeviceBrowser) {self.ivars().borrow_mut().enumerated=Some(Instant::now());}
    }
);
impl BrowserDelegate {
    pub fn new(marker: MainThreadMarker, state: Rc<RefCell<BrowserSignals>>) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(state);
        // SAFETY: initialized ivars, NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}
define_class!(
    // SAFETY: retained by Connection, main-thread callbacks with the SDK signatures.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "ScannerRustSessionDelegate"]
    #[ivars = Rc<RefCell<Signals>>]
    pub(super) struct SessionDelegate;
    unsafe impl NSObjectProtocol for SessionDelegate {}
    unsafe impl ICDeviceDelegate for SessionDelegate {
        #[unsafe(method(device:didOpenSessionWithError:))]
        fn opened(&self,_device:&ICDevice,error:Option<&NSError>) {
            let mut state=self.ivars().borrow_mut();state.open_completed=true;
            if let Some(error)=convert("openSession",error) {state.error=Some(error);} else {state.opened=true;}
        }
        #[unsafe(method(device:didCloseSessionWithError:))]
        fn closed(&self,_device:&ICDevice,error:Option<&NSError>) {
            let mut state=self.ivars().borrow_mut();state.close_completed=true;
            if let Some(error)=convert("closeSession",error) {state.error=Some(error);} else {state.closed=true;}
            if !state.closing {state.error=Some(ScannerError::new("disconnected","Scanner session closed unexpectedly."));}
        }
        #[unsafe(method(didRemoveDevice:))]
        fn removed(&self,device:&ICDevice) {self.ivars().borrow_mut().remove(identity(device));}
        #[unsafe(method(deviceDidBecomeReady:))]
        fn ready(&self,_device:&ICDevice) {self.ivars().borrow_mut().ready=true;}
        #[unsafe(method(device:didEncounterError:))]
        fn error(&self,_device:&ICDevice,error:Option<&NSError>) {if let Some(error)=convert("driver",error){self.ivars().borrow_mut().error=Some(error);}}
    }
    unsafe impl ICScannerDeviceDelegate for SessionDelegate {
        #[unsafe(method(scannerDevice:didSelectFunctionalUnit:error:))]
        fn selected(&self,_scanner:&ICScannerDevice,_unit:&ICScannerFunctionalUnit,error:Option<&NSError>) {
            let mut state=self.ivars().borrow_mut();state.selected=true;
            if let Some(error)=convert("selectUnit",error) {state.error=Some(error);}
        }
        #[unsafe(method(scannerDevice:didScanToURL:))]
        fn file(&self,scanner:&ICScannerDevice,url:&NSURL) {
            let path=if url.isFileURL(){url.path().map(|s|PathBuf::from(s.to_string()))} else {None};
            let cancel=self.ivars().borrow_mut().receive(path);
            // Release the RefCell borrow before invoking a potentially reentrant driver.
            if cancel {unsafe {scanner.cancelScan();}}
        }
        #[unsafe(method(scannerDevice:didCompleteScanWithError:))]
        fn completed(&self,_scanner:&ICScannerDevice,error:Option<&NSError>) {
            let mut state=self.ivars().borrow_mut();state.scan_completed=true;
            if let Some(error)=convert("scan",error) {state.error=Some(error);}
        }
    }
    impl SessionDelegate {
        #[unsafe(method(device:didReceiveButtonPress:))]
        fn button(&self,device:&ICDevice,button:&objc2_foundation::NSString) {
            if !self.ivars().borrow().button_armed {return;}
            let loaded = unsafe {
                button == objc2_image_capture_core::ICButtonTypeScan && device.downcast_ref::<ICScannerDevice>()
                    .and_then(|scanner|scanner.selectedFunctionalUnit().downcast::<objc2_image_capture_core::ICScannerFunctionalUnitDocumentFeeder>().ok())
                    .is_some_and(|feeder|feeder.documentLoaded() && !feeder.scanInProgress())
            };
            if loaded {self.ivars().borrow_mut().button_accepted=true;}
        }
    }

);
impl SessionDelegate {
    pub fn new(marker: MainThreadMarker, state: Rc<RefCell<Signals>>) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(state);
        unsafe { msg_send![super(this), init] }
    }
}
fn convert(kind: &str, error: Option<&NSError>) -> Option<ScannerError> {
    error.map(|error| ScannerError {
        kind: kind.into(),
        message: error.localizedDescription().to_string(),
        domain: Some(error.domain().to_string()),
        code: Some(error.code() as i64),
    })
}
