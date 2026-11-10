//! Hand-written Windows TWAIN 2 ABI subset. Layouts/constants follow the TWAIN
//! Working Group's public twain.h interface, including its two-byte packing.
use std::ffi::c_void;
pub(super) type Entry =
    unsafe extern "system" fn(*mut Identity, *mut Identity, u32, u16, u16, *mut c_void) -> u16;
pub(super) type Allocate = unsafe extern "system" fn(u32) -> *mut c_void;
pub(super) type Free = unsafe extern "system" fn(*mut c_void);
pub(super) type Lock = unsafe extern "system" fn(*mut c_void) -> *mut c_void;
pub(super) type Unlock = unsafe extern "system" fn(*mut c_void);
#[repr(C, packed(2))]
#[derive(Clone, Copy)]
pub(super) struct Version {
    pub major: u16,
    pub minor: u16,
    pub language: u16,
    pub country: u16,
    pub info: [u8; 34],
}
#[repr(C, packed(2))]
#[derive(Clone, Copy)]
pub(super) struct Identity {
    pub id: u32,
    pub version: Version,
    pub protocol_major: u16,
    pub protocol_minor: u16,
    pub groups: u32,
    pub manufacturer: [u8; 34],
    pub family: [u8; 34],
    pub product: [u8; 34],
}
impl Identity {
    pub fn empty() -> Self {
        Self {
            id: 0,
            version: Version {
                major: 0,
                minor: 0,
                language: 0,
                country: 0,
                info: [0; 34],
            },
            protocol_major: 0,
            protocol_minor: 0,
            groups: 0,
            manufacturer: [0; 34],
            family: [0; 34],
            product: [0; 34],
        }
    }
    pub fn application() -> Self {
        let mut value = Self::empty();
        value.version.major = 1;
        value.version.language = 13;
        value.version.country = 44;
        value.version.info = text("Scanner Rust");
        value.protocol_major = 2;
        value.protocol_minor = 5;
        value.groups = 0x20000003;
        value.manufacturer = text("Scanner");
        value.family = text("Card scanner");
        value.product = text("Scanner");
        value
    }
}
fn text(value: &str) -> [u8; 34] {
    let mut bytes = [0; 34];
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    bytes
}
pub(super) fn string(value: [u8; 34]) -> String {
    String::from_utf8_lossy(&value[..value.iter().position(|b| *b == 0).unwrap_or(34)]).into_owned()
}
#[repr(C, packed(2))]
pub(super) struct EntryPoints {
    pub size: u32,
    pub entry: Option<Entry>,
    pub allocate: Option<Allocate>,
    pub free: Option<Free>,
    pub lock: Option<Lock>,
    pub unlock: Option<Unlock>,
}
#[repr(C, packed(2))]
pub(super) struct Capability {
    pub id: u16,
    pub container_type: u16,
    pub handle: *mut c_void,
}
#[repr(C, packed(2))]
#[derive(Default)]
pub(super) struct Status {
    pub condition: u16,
    pub data: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_abi_packing_matches_twain_header() {
        assert_eq!(std::mem::size_of::<Version>(), 42);
        assert_eq!(std::mem::size_of::<Identity>(), 156);
        assert_eq!(std::mem::align_of::<Identity>(), 2);
        assert_eq!(std::mem::offset_of!(Identity, groups), 50);
        assert_eq!(
            std::mem::size_of::<EntryPoints>(),
            4 + 5 * std::mem::size_of::<usize>()
        );
        assert_eq!(
            std::mem::size_of::<Capability>(),
            4 + std::mem::size_of::<usize>()
        );
    }
}
