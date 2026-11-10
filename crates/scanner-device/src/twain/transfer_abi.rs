use std::ffi::c_void;
#[repr(C, packed(2))]
#[derive(Clone, Copy, Default)]
pub(super) struct Fix32 {
    pub whole: i16,
    pub fraction: u16,
}
#[repr(C, packed(2))]
#[derive(Clone, Copy, Default)]
pub(super) struct ImageInfo {
    pub x_resolution: Fix32,
    pub y_resolution: Fix32,
    pub width: i32,
    pub height: i32,
    pub samples: i16,
    pub bits: [i16; 8],
    pub bits_per_pixel: i16,
    pub planar: u16,
    pub pixel_type: i16,
    pub compression: u16,
}
#[repr(C, packed(2))]
#[derive(Default)]
pub(super) struct Memory {
    pub flags: u32,
    pub length: u32,
    pub pointer: *mut c_void,
}
#[repr(C, packed(2))]
#[derive(Default)]
pub(super) struct ImageTransfer {
    pub compression: u16,
    pub stride: u32,
    pub columns: u32,
    pub rows: u32,
    pub x_offset: u32,
    pub y_offset: u32,
    pub bytes_written: u32,
    pub memory: Memory,
}
#[repr(C, packed(2))]
#[derive(Default)]
pub(super) struct BufferSizes {
    pub minimum: u32,
    pub maximum: u32,
    pub preferred: u32,
}
#[repr(C, packed(2))]
#[derive(Default)]
pub(super) struct Pending {
    pub count: u16,
    pub reserved: u32,
}
#[repr(C, packed(2))]
pub(super) struct UserInterface {
    pub show: u16,
    pub modal: u16,
    pub parent: *mut c_void,
}
#[repr(C, packed(2))]
pub(super) struct Event {
    pub native: *mut c_void,
    pub message: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transfer_structures_follow_twain_two_byte_packing() {
        assert_eq!(std::mem::size_of::<ImageInfo>(), 42);
        assert_eq!(
            std::mem::size_of::<Memory>(),
            8 + std::mem::size_of::<usize>()
        );
        assert_eq!(
            std::mem::size_of::<ImageTransfer>(),
            34 + std::mem::size_of::<usize>()
        );
        assert_eq!(std::mem::offset_of!(ImageTransfer, memory), 26);
        assert_eq!(std::mem::size_of::<Pending>(), 6);
    }
}
