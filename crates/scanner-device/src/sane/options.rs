use super::{
    ffi::{self, Api, Handle},
    value::{Data, OptionInfo},
};
use scanner_types::error::Result;

pub(super) struct Options<'a> {
    pub api: &'a Api,
    pub handle: Handle,
}
pub(super) trait OptionAccess {
    fn list(&self) -> Result<Vec<OptionInfo>>;
    fn get(&self, option: &OptionInfo) -> Result<Data>;
    fn set(&self, option: &OptionInfo, value: &Data) -> Result<()>;
    fn find(&self, name: &str) -> Result<Option<OptionInfo>> {
        Ok(self
            .list()?
            .into_iter()
            .find(|o| o.name == name && o.active()))
    }
}
impl OptionAccess for Options<'_> {
    fn list(&self) -> Result<Vec<OptionInfo>> {
        let mut count = 0i32;
        // SAFETY: this worker owns an open handle; option 0 is a SANE_Int count.
        unsafe {
            ffi::check(
                (self.api.control)(
                    self.handle,
                    0,
                    0,
                    (&mut count as *mut i32).cast(),
                    std::ptr::null_mut(),
                ),
                "read option count",
            )?;
        }
        if !(1..=512).contains(&count) {
            return Err(ffi::error("Invalid SANE option count."));
        }
        (1..count)
            .map(|index| {
                // SAFETY: descriptor copied immediately; handle remains open.
                unsafe {
                    let raw = (self.api.descriptor)(self.handle, index)
                        .as_ref()
                        .ok_or_else(|| ffi::error("Missing SANE descriptor."))?;
                    OptionInfo::copy(index, raw)
                }
            })
            .collect()
    }
    fn get(&self, option: &OptionInfo) -> Result<Data> {
        if !option.readable() || option.size == 0 {
            return Err(ffi::error("SANE option is not readable."));
        }
        let mut buffer = vec![0i32; option.size.div_ceil(4)];
        // SAFETY: word-aligned allocation holds at least the descriptor's size.
        unsafe {
            ffi::check(
                (self.api.control)(
                    self.handle,
                    option.index,
                    0,
                    buffer.as_mut_ptr().cast(),
                    std::ptr::null_mut(),
                ),
                "read option",
            )?;
        }
        if option.kind == 3 {
            let bytes: Vec<u8> = buffer
                .iter()
                .flat_map(|v| v.to_ne_bytes())
                .take(option.size)
                .collect();
            let end = bytes
                .iter()
                .position(|&v| v == 0)
                .ok_or_else(|| ffi::error("Unterminated SANE option string."))?;
            return Ok(Data::Text(
                String::from_utf8(bytes[..end].to_vec()).map_err(ffi::error)?,
            ));
        }
        if !option.size.is_multiple_of(4) {
            return Err(ffi::error("Invalid SANE word size."));
        }
        Ok(Data::Words(buffer))
    }
    fn set(&self, option: &OptionInfo, value: &Data) -> Result<()> {
        if !option.writable() || !option.accepts(value) {
            return Err(ffi::error(format!(
                "Unsupported value for {}.",
                option.name
            )));
        }
        let mut buffer = encode(option, value)?;
        let mut info = 0;
        // SAFETY: writable aligned allocation matches the descriptor. SET may
        // change descriptors; callers resolve names again before every operation.
        unsafe {
            ffi::check(
                (self.api.control)(
                    self.handle,
                    option.index,
                    1,
                    buffer.as_mut_ptr().cast(),
                    &mut info,
                ),
                "write option",
            )?;
        }
        if info & 1 != 0 {
            return Err(ffi::error(format!(
                "Driver adjusted requested {}.",
                option.name
            )));
        }
        Ok(())
    }
}
fn encode(option: &OptionInfo, value: &Data) -> Result<Vec<i32>> {
    match value {
        Data::Words(words) if option.kind <= 2 && words.len() * 4 == option.size => {
            Ok(words.clone())
        }
        Data::Text(text)
            if option.kind == 3 && text.len() < option.size && !text.contains('\0') =>
        {
            let mut bytes = vec![0u8; option.size.div_ceil(4) * 4];
            bytes[..text.len()].copy_from_slice(text.as_bytes());
            Ok(bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|v| i32::from_ne_bytes(*v))
                .collect())
        }
        _ => Err(ffi::error("SANE option value has the wrong type or size.")),
    }
}
