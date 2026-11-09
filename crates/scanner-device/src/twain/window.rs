use super::manager::error;
use scanner_types::error::Result;
use windows_sys::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DispatchMessageW, MSG, PM_REMOVE, PeekMessageW,
        TranslateMessage,
    },
};

pub(super) struct Window(pub HWND);
impl Window {
    pub fn new() -> Result<Self> {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let title: Vec<u16> = "Scanner TWAIN\0".encode_utf16().collect();
        // SAFETY: predefined window class, NUL-terminated strings, no visible
        // style. The owning worker pumps messages and destroys it on that thread.
        let handle = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                0,
                0,
                0,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        if handle.is_null() {
            return Err(error(std::io::Error::last_os_error()));
        }
        Ok(Self(handle))
    }
    pub fn pump(&self) {
        let mut message = MSG::default();
        for _ in 0..128 {
            // SAFETY: local MSG storage; dispatch only this worker's queue.
            unsafe {
                if PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) == 0 {
                    break;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.0);
        }
    }
}
