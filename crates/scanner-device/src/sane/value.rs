use super::ffi::{self, Descriptor};
use scanner_types::error::Result;
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Data {
    Words(Vec<i32>),
    Text(String),
}
#[derive(Clone, Debug)]
pub(super) enum Constraint {
    None,
    Range { min: i32, max: i32, step: i32 },
    Words(Vec<i32>),
    Text(Vec<String>),
}
#[derive(Clone, Debug)]
pub(super) struct OptionInfo {
    pub index: i32,
    pub name: String,
    pub title: String,
    pub kind: i32,
    pub unit: i32,
    pub size: usize,
    pub caps: i32,
    pub constraint: Constraint,
}
impl OptionInfo {
    /// The caller must supply a live descriptor from the current SANE handle.
    pub unsafe fn copy(index: i32, raw: &Descriptor) -> Result<Self> {
        if !(0..=65536).contains(&raw.size) || !(0..=5).contains(&raw.kind) {
            return Err(ffi::error("Invalid SANE option size or type."));
        }
        let constraint = unsafe { copy_constraint(raw)? };
        Ok(Self {
            index,
            name: unsafe { ffi::string(raw.name)? },
            title: unsafe { ffi::string(raw.title)? },
            kind: raw.kind,
            unit: raw.unit,
            size: raw.size as usize,
            caps: raw.capabilities,
            constraint,
        })
    }
    pub fn active(&self) -> bool {
        self.caps & 32 == 0
    }
    pub fn readable(&self) -> bool {
        self.active() && self.caps & 4 != 0 && self.kind <= 3
    }
    pub fn writable(&self) -> bool {
        self.readable() && self.caps & 1 != 0
    }
    pub fn number(&self, number: f64) -> Result<Data> {
        if self.size != 4 || !matches!(self.kind, 0..=2) || !number.is_finite() {
            return Err(ffi::error(format!("{} is not a scalar number.", self.name)));
        }
        let scaled = if self.kind == 2 {
            number * 65536.0
        } else {
            number
        };
        if scaled < i32::MIN as f64
            || scaled > i32::MAX as f64
            || (scaled.round() - scaled).abs() > 0.001
        {
            return Err(ffi::error("SANE number cannot be represented exactly."));
        }
        Ok(Data::Words(vec![scaled.round() as i32]))
    }
    pub fn json(&self, data: &Data) -> Value {
        match data {
            Data::Text(text) => json!(text),
            Data::Words(words) => {
                let values: Vec<_> = words
                    .iter()
                    .map(|&word| match self.kind {
                        0 => json!(word != 0),
                        2 => json!(word as f64 / 65536.0),
                        _ => json!(word),
                    })
                    .collect();
                if values.len() == 1 {
                    values[0].clone()
                } else {
                    json!(values)
                }
            }
        }
    }
    pub fn allowed(&self) -> Value {
        match &self.constraint {
            Constraint::None => Value::Null,
            Constraint::Text(values) => json!(values),
            Constraint::Words(values) => self.json(&Data::Words(values.clone())),
            Constraint::Range { min, max, step } => json!({
                "minimum": self.json(&Data::Words(vec![*min])),
                "maximum": self.json(&Data::Words(vec![*max])),
                "step": self.json(&Data::Words(vec![*step])),
            }),
        }
    }
    pub fn accepts(&self, data: &Data) -> bool {
        match (&self.constraint, data) {
            (Constraint::None, _) => true,
            (Constraint::Text(allowed), Data::Text(value)) => allowed.contains(value),
            (Constraint::Words(allowed), Data::Words(values)) => {
                values.iter().all(|v| allowed.contains(v))
            }
            (Constraint::Range { min, max, step }, Data::Words(values)) => values.iter().all(|v| {
                v >= min
                    && v <= max
                    && (*step <= 0 || (i64::from(*v) - i64::from(*min)) % i64::from(*step) == 0)
            }),
            _ => false,
        }
    }
}
unsafe fn copy_constraint(raw: &Descriptor) -> Result<Constraint> {
    // SAFETY: these tagged union pointers belong to the live SANE descriptor;
    // every value is copied before another API call can invalidate the storage.
    unsafe {
        Ok(match raw.constraint_kind {
            0 => Constraint::None,
            1 => {
                let range = raw
                    .constraint
                    .range
                    .as_ref()
                    .ok_or_else(|| ffi::error("Missing SANE range."))?;
                if range.min > range.max || range.quant < 0 {
                    return Err(ffi::error("Invalid SANE range."));
                }
                Constraint::Range {
                    min: range.min,
                    max: range.max,
                    step: range.quant,
                }
            }
            2 => {
                let list = raw.constraint.words;
                if list.is_null() || !(0..=256).contains(&*list) {
                    return Err(ffi::error("Invalid SANE word list."));
                }
                Constraint::Words(std::slice::from_raw_parts(list.add(1), *list as usize).to_vec())
            }
            3 => {
                let list = raw.constraint.strings;
                if list.is_null() {
                    return Err(ffi::error("Missing SANE string list."));
                }
                let mut values = Vec::new();
                for index in 0..=256 {
                    let pointer = *list.add(index);
                    if pointer.is_null() {
                        return Ok(Constraint::Text(values));
                    }
                    if index == 256 {
                        return Err(ffi::error("Oversized SANE string list."));
                    }
                    values.push(ffi::string(pointer)?);
                }
                unreachable!()
            }
            _ => return Err(ffi::error("Unknown SANE constraint.")),
        })
    }
}
