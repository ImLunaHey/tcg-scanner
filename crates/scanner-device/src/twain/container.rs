use scanner_types::error::{Result, ScannerError};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Scalar {
    pub kind: u16,
    pub raw: u32,
}
impl Scalar {
    pub fn json(self) -> Value {
        match self.kind {
            0 => json!(self.raw as i8),
            1 => json!(self.raw as i16),
            2 => json!(self.raw as i32),
            3 => json!(self.raw as u8),
            4 => json!(self.raw as u16),
            5 => json!(self.raw),
            6 => json!(self.raw != 0),
            // TW_FIX32 stores Whole then Frac, rather than an integer's bitwise
            // 16.16 representation. Both Windows architectures are little-endian.
            7 => json!(self.raw as i16 as f64 + (self.raw >> 16) as f64 / 65536.0),
            _ => Value::Null,
        }
    }
}
pub(super) fn item_size(kind: u16) -> Result<usize> {
    match kind {
        0 | 3 => Ok(1),
        1 | 4 | 6 => Ok(2),
        2 | 5 | 7 => Ok(4),
        _ => Err(invalid("Unsupported TWAIN capability item type.")),
    }
}
pub(super) fn current(container_type: u16, bytes: &[u8]) -> Result<Scalar> {
    let kind = word(bytes, 0)?;
    let size = item_size(kind)?;
    let offset = match container_type {
        5 => 2,
        6 => 18,
        4 => {
            let count = dword(bytes, 2)? as usize;
            let index = dword(bytes, 6)? as usize;
            if count == 0 || count > 4096 || index >= count || bytes.len() < 14 + count * size {
                return Err(invalid("Invalid TWAIN enumeration."));
            }
            14 + index * size
        }
        _ => return Err(invalid("Current TWAIN capability is not scalar.")),
    };
    let value = bytes
        .get(offset..offset + size)
        .ok_or_else(|| invalid("Truncated TWAIN capability."))?;
    let mut raw = [0; 4];
    raw[..size].copy_from_slice(value);
    Ok(Scalar {
        kind,
        raw: u32::from_le_bytes(raw),
    })
}
pub(super) fn word(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or_else(|| invalid("Truncated TWAIN header."))?
            .try_into()
            .unwrap(),
    ))
}
pub(super) fn dword(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or_else(|| invalid("Truncated TWAIN header."))?
            .try_into()
            .unwrap(),
    ))
}
fn invalid(message: &str) -> ScannerError {
    ScannerError::new("driver", message)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn respects_enum_item_width_and_fixed_point_byte_order() {
        let mut bytes = vec![4, 0];
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&[2, 0, 9, 0]);
        assert_eq!(current(4, &bytes).unwrap().json(), 9);
        assert_eq!(current(5, &[7, 0, 254, 255, 0, 128]).unwrap().json(), -1.5);
        assert_eq!(current(5, &[7, 0, 88, 2, 0, 0]).unwrap().json(), 600.0);
        assert_eq!(current(5, &[1, 0, 255, 255, 0, 0]).unwrap().json(), -1);
    }
    #[test]
    fn rejects_truncated_and_out_of_range_enumerations() {
        assert!(current(5, &[5, 0, 1]).is_err());
        let mut bytes = vec![4, 0];
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&[0, 0]);
        assert!(current(4, &bytes).is_err());
        assert!(current(3, &bytes).is_err());
    }
}
