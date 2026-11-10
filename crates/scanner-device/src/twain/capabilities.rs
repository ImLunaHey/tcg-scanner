use super::{
    abi::Capability,
    container::{self, Scalar},
    manager::{Manager, Memory, error},
};
use scanner_types::error::Result;
use std::ffi::c_void;

pub(super) const INSPECTION_CAPS: &[(u16, &str)] = &[
    (0x1002, "feederEnabled"),
    (0x1003, "feederLoaded"),
    (0x1012, "duplex"),
    (0x1013, "duplexEnabled"),
    (0x0101, "pixelType"),
    (0x0102, "units"),
    (0x0103, "transferMechanism"),
    (0x112b, "bitDepth"),
    (0x1118, "xResolution"),
    (0x1119, "yResolution"),
    (0x1122, "paperSize"),
    (0x1134, "blankPageDiscard"),
    (0x1149, "overscan"),
    (0x1150, "automaticBorderDetection"),
    (0x1151, "automaticDeskew"),
    (0x1152, "automaticRotate"),
];
struct Container {
    memory: Memory,
    handle: *mut c_void,
    locked: bool,
}
impl Drop for Container {
    fn drop(&mut self) {
        unsafe {
            if self.locked {
                (self.memory.unlock)(self.handle);
            }
            (self.memory.free)(self.handle);
        }
    }
}
pub(super) fn read(manager: &mut Manager, id: u16, message: u16) -> Result<Scalar> {
    read_container(manager, id, message, container::current)
}
pub(super) fn allowed(manager: &mut Manager, id: u16) -> Result<serde_json::Value> {
    read_container(manager, id, 1, container::allowed)
}
fn read_container<T>(
    manager: &mut Manager,
    id: u16,
    message: u16,
    decode: impl FnOnce(u16, &[u8]) -> Result<T>,
) -> Result<T> {
    let mut capability = Capability {
        id,
        container_type: 0,
        handle: std::ptr::null_mut(),
    };
    let result = manager.source_call(1, 1, message, (&mut capability as *mut Capability).cast());
    let memory = manager
        .memory
        .ok_or_else(|| error("DSM memory functions unavailable."))?;
    if capability.handle.is_null() {
        return result.and_then(|()| Err(error("Empty TWAIN capability container.")));
    }
    let mut owned = Container {
        memory,
        handle: capability.handle,
        locked: false,
    };
    result?;
    // SAFETY: the trusted native DSM returned this owned capability handle. Its
    // tagged allocation follows twain.h. Copy bounded scalar data while locked.
    let pointer = unsafe { (memory.lock)(owned.handle) }.cast::<u8>();
    if pointer.is_null() {
        return Err(error("Could not lock TWAIN capability."));
    }
    owned.locked = true;
    unsafe {
        let length = match capability.container_type {
            5 => 6,
            6 => 22,
            3 | 4 => {
                let header_len = if capability.container_type == 3 {
                    6
                } else {
                    14
                };
                let header = std::slice::from_raw_parts(pointer, header_len);
                let count = container::dword(header, 2)? as usize;
                if count > 4096 {
                    return Err(error("Oversized TWAIN capability."));
                }
                header_len + count * container::item_size(container::word(header, 0)?)?
            }
            _ => return Err(error("Unsupported TWAIN capability container.")),
        };
        decode(
            capability.container_type,
            std::slice::from_raw_parts(pointer, length),
        )
    }
}

pub(super) fn write(manager: &mut Manager, id: u16, value: Scalar) -> Result<()> {
    let memory = manager
        .memory
        .ok_or_else(|| error("DSM memory functions unavailable."))?;
    // SAFETY: six bytes hold a TW_ONEVALUE for the supported scalar types.
    let handle = unsafe { (memory.allocate)(6) };
    if handle.is_null() {
        return Err(error("Could not allocate TWAIN setting container."));
    }
    let mut owned = Container {
        memory,
        handle,
        locked: false,
    };
    let pointer = unsafe { (memory.lock)(handle) }.cast::<u8>();
    if pointer.is_null() {
        return Err(error("Could not lock TWAIN setting container."));
    }
    owned.locked = true;
    let mut bytes = [0; 6];
    bytes[..2].copy_from_slice(&value.kind.to_le_bytes());
    bytes[2..].copy_from_slice(&value.raw.to_le_bytes());
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, 6);
        (memory.unlock)(handle);
    }
    owned.locked = false;
    let mut capability = Capability {
        id,
        container_type: 5,
        handle,
    };
    manager.source_call(1, 1, 6, (&mut capability as *mut Capability).cast())
}
