use super::{
    ffi,
    options::OptionAccess,
    value::{Constraint, Data},
};
use scanner_types::{
    Extensions,
    error::{Result, ScannerError},
    scan::{ScanBackground, ScanRequest},
};

const NAMES: &[&str] = &[
    "source",
    "mode",
    "depth",
    "resolution",
    "y-resolution",
    "page-width",
    "page-height",
    "tl-x",
    "tl-y",
    "br-x",
    "br-y",
    "swskip",
    "swcrop",
    "swdeskew",
    "swdespeck",
    "bgcolor",
    "overscan",
];
#[derive(Default)]
pub(super) struct Changes {
    originals: Vec<(String, Data)>,
    changed: Vec<String>,
    initial_count: usize,
}
impl Changes {
    pub fn snapshot(&mut self, options: &impl OptionAccess) -> Result<()> {
        for &name in NAMES {
            if let Some(option) = options.find(name)?.filter(|o| o.readable()) {
                self.originals.push((name.into(), options.get(&option)?));
            }
        }
        self.initial_count = self.originals.len();
        Ok(())
    }
    fn set(&mut self, options: &impl OptionAccess, name: &str, value: Data) -> Result<()> {
        let option = options.find(name)?.ok_or_else(|| unsupported(name))?;
        let old = options.get(&option)?;
        if old == value {
            return Ok(());
        }
        if !option.writable() || !option.accepts(&value) {
            return Err(unsupported(name));
        }
        if !self.originals.iter().any(|(key, _)| key == name) {
            self.originals.push((name.into(), old));
        }
        if !self.changed.iter().any(|key| key == name) {
            self.changed.push(name.into());
        }
        // Record before writing: even an error may leave a partially applied value.
        options.set(&option, &value)?;
        let refreshed = options.find(name)?.ok_or_else(|| unsupported(name))?;
        if options.get(&refreshed)? != value {
            return Err(ffi::error(format!("SANE did not apply {name}.")));
        }
        Ok(())
    }
    fn number(
        &mut self,
        options: &impl OptionAccess,
        name: &str,
        number: f64,
        unit: i32,
        required: bool,
    ) -> Result<()> {
        let Some(option) = options.find(name)? else {
            return if required {
                Err(unsupported(name))
            } else {
                Ok(())
            };
        };
        if option.unit != unit {
            return Err(ffi::error(format!("Unexpected units for {name}.")));
        }
        self.set(options, name, option.number(number)?)
    }
    pub fn configure(
        &mut self,
        options: &impl OptionAccess,
        request: &ScanRequest,
    ) -> Result<Extensions> {
        request.validate()?;
        let source = options
            .find("source")?
            .ok_or_else(|| unsupported("ADF source"))?;
        let choices = if request.duplex {
            &["ADF Duplex"][..]
        } else {
            &["ADF Front", "ADF"][..]
        };
        let selected = match &source.constraint {
            Constraint::Text(allowed) => choices
                .iter()
                .find(|value| allowed.iter().any(|a| a == **value)),
            _ => None,
        }
        .ok_or_else(|| unsupported("requested ADF source"))?;
        self.set(options, "source", Data::Text((*selected).into()))?;
        self.set(options, "mode", Data::Text("Color".into()))?;
        self.number(options, "depth", 8.0, 2, false)?;
        self.number(options, "resolution", request.dpi as f64, 4, true)?;
        self.number(options, "y-resolution", request.dpi as f64, 4, false)?;
        for (name, number, required) in [
            ("page-width", 210.0, false),
            ("page-height", 297.0, false),
            ("tl-x", 0.0, true),
            ("tl-y", 0.0, true),
            ("br-x", 210.0, true),
            ("br-y", 297.0, true),
        ] {
            self.number(options, name, number, 3, required)?;
        }
        for (name, unit) in [
            ("swskip", 5),
            ("swcrop", 0),
            ("swdeskew", 0),
            ("swdespeck", 0),
        ] {
            self.number(options, name, 0.0, unit, false)?;
        }
        if let Some(background) = request.background.or(request
            .overscan
            .filter(|v| *v)
            .map(|_| ScanBackground::Black))
        {
            self.set(
                options,
                "bgcolor",
                Data::Text(
                    match background {
                        ScanBackground::Black => "Black",
                        ScanBackground::White => "White",
                    }
                    .into(),
                ),
            )?;
        }
        if let Some(overscan) = request.overscan {
            self.set(
                options,
                "overscan",
                Data::Text(if overscan { "On" } else { "Off" }.into()),
            )?;
        }
        let mut applied = Extensions::new();
        for &name in NAMES {
            if let Some(option) = options.find(name)?.filter(|o| o.readable()) {
                applied.insert(name.into(), option.json(&options.get(&option)?));
            }
        }
        applied.insert("backend".into(), "sane".into());
        Ok(applied)
    }
    pub fn restore(&self, options: &impl OptionAccess) -> Vec<ScannerError> {
        if self.changed.is_empty() {
            return vec![];
        }
        // Restore selectors first, then their dependent values from the initial
        // snapshot. A selector can reset options we never explicitly changed.
        let mut errors = Vec::new();
        for (index, (name, original)) in self.originals.iter().enumerate() {
            let result = (|| {
                let Some(option) = options.find(name)? else {
                    return if index < self.initial_count {
                        Err(unsupported(name))
                    } else {
                        Ok(())
                    };
                };
                if !option.readable() {
                    return Err(unsupported(name));
                }
                if options.get(&option)? == *original {
                    return Ok(());
                }
                options.set(&option, original)?;
                let restored = options.find(name)?.ok_or_else(|| unsupported(name))?;
                if options.get(&restored)? != *original {
                    return Err(ffi::error("Settings restoration readback differed."));
                }
                Ok(())
            })();
            if let Err(error) = result {
                errors.push(ScannerError::new(
                    "cleanup",
                    format!("Could not restore {name}: {error}"),
                ));
            }
        }
        errors
    }
}
fn unsupported(name: &str) -> ScannerError {
    ScannerError::new("unsupported", format!("SANE driver cannot provide {name}."))
}
