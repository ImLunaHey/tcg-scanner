use scanner_types::{
    device::{Inspection, Setting, UnitSnapshot},
    error::{Result, ScannerError},
    service::ScanSettings,
};
use serde_json::Value;

pub(crate) fn validate(report: &Inspection, settings: &ScanSettings) -> Result<()> {
    let supported = report.units.iter().any(|unit| {
        if unit.unit_type == 3 {
            number(unit.settings.get("resolutionDPI"), settings.dpi)
                && (!settings.duplex
                    || current(unit, "supportsDuplexScanning") == Some(&Value::Bool(true)))
        } else if unit.class_name == "SANE" {
            let source = unit.settings.get("source");
            let choices = source
                .and_then(|s| s.allowed.as_ref())
                .and_then(Value::as_array);
            let feeder = choices.is_some_and(|values| {
                values.iter().any(|v| {
                    let s = v.as_str().unwrap_or_default();
                    if settings.duplex {
                        s == "ADF Duplex"
                    } else {
                        matches!(s, "ADF Front" | "ADF")
                    }
                })
            });
            let dpi_units = unit
                .vendor_features
                .iter()
                .any(|v| v["name"] == "resolution" && v["saneUnit"] == 4);
            feeder
                && dpi_units
                && number(unit.settings.get("resolution"), settings.dpi)
                && unit
                    .settings
                    .get("y-resolution")
                    .is_none_or(|s| number(Some(s), settings.dpi))
        } else if unit.class_name == "TWAIN" {
            unit.settings.contains_key("feederEnabled")
                && current(unit, "units").and_then(Value::as_u64) == Some(0)
                && number(unit.settings.get("xResolution"), settings.dpi)
                && number(unit.settings.get("yResolution"), settings.dpi)
                && (!settings.duplex
                    || current(unit, "duplex")
                        .and_then(Value::as_u64)
                        .is_some_and(|v| v == 1 || v == 2))
        } else {
            false
        }
    });
    if supported {
        Ok(())
    } else {
        Err(ScannerError::new(
            "unsupportedSetting",
            "Inspection does not expose the requested ADF resolution and duplex support.",
        ))
    }
}
pub(crate) fn loaded(report: &Inspection) -> Option<bool> {
    report.units.iter().find_map(|u| {
        let key = if u.unit_type == 3 {
            "documentLoaded"
        } else if u.class_name == "TWAIN" {
            "feederLoaded"
        } else {
            return None;
        };
        current(u, key).and_then(Value::as_bool)
    })
}
fn current<'a>(unit: &'a UnitSnapshot, key: &str) -> Option<&'a Value> {
    unit.settings.get(key).map(|s| &s.current)
}
fn number(setting: Option<&Setting>, dpi: u32) -> bool {
    let Some(setting) = setting else {
        return false;
    };
    let dpi = f64::from(dpi);
    if setting.read_only {
        return setting.current.as_f64() == Some(dpi);
    }
    let Some(allowed) = setting.allowed.as_ref() else {
        return false;
    };
    if let Some(list) = allowed.as_array() {
        return list.iter().any(|v| v.as_f64() == Some(dpi));
    }
    // A single SANE word constraint serializes as a scalar.
    if allowed.as_f64() == Some(dpi) {
        return true;
    }
    let (Some(min), Some(max), Some(step)) = (
        allowed["minimum"].as_f64(),
        allowed["maximum"].as_f64(),
        allowed["step"].as_f64(),
    ) else {
        return false;
    };
    min.is_finite()
        && max.is_finite()
        && step.is_finite()
        && step >= 0.0
        && dpi >= min
        && dpi <= max
        && (step == 0.0 || ((dpi - min) / step).fract().abs() < 1e-8)
}
#[cfg(test)]
mod tests {
    use super::*;
    use scanner_types::device::Evidence;
    use serde_json::json;
    #[test]
    fn portable_backends_require_explicit_feeder_duplex_and_dpi_evidence() {
        let setting = |current, allowed| Setting {
            current,
            allowed: Some(allowed),
            default_value: None,
            read_only: false,
            evidence: Evidence::DriverExposedNotTested,
        };
        let settings = ScanSettings {
            dpi: 600,
            duplex: true,
            format: scanner_types::scan::OriginalFormat::Png,
        };
        let mut report = Inspection::new(Default::default());
        report.units.push(UnitSnapshot {
            class_name: "SANE".into(),
            settings: [
                (
                    "source".into(),
                    setting(json!("Flatbed"), json!(["ADF Front", "ADF Duplex"])),
                ),
                (
                    "resolution".into(),
                    setting(json!(300), json!({"minimum":150,"maximum":600,"step":150})),
                ),
            ]
            .into(),
            vendor_features: vec![json!({"name":"resolution","saneUnit":4})],
            ..Default::default()
        });
        assert!(validate(&report, &settings).is_ok());
        assert_eq!(loaded(&report), None);
        report.units[0].vendor_features.clear();
        assert!(validate(&report, &settings).is_err());
        report.units[0] = UnitSnapshot {
            class_name: "TWAIN".into(),
            settings: [
                (
                    "feederEnabled".into(),
                    setting(json!(false), json!([true, false])),
                ),
                ("feederLoaded".into(), setting(json!(true), Value::Null)),
                ("duplex".into(), setting(json!(1), Value::Null)),
                ("units".into(), setting(json!(0), Value::Null)),
                ("xResolution".into(), setting(json!(300), json!([300, 600]))),
                ("yResolution".into(), setting(json!(300), json!([300, 600]))),
            ]
            .into(),
            ..Default::default()
        };
        assert!(validate(&report, &settings).is_ok());
        assert_eq!(loaded(&report), Some(true));
        report.units[0].settings.get_mut("units").unwrap().current = json!(1);
        assert!(validate(&report, &settings).is_err());
    }
    #[test]
    fn bounds_step_and_read_only_values_are_respected() {
        let mut setting = Setting {
            current: json!(300),
            allowed: Some(json!({"minimum":150,"maximum":600,"step":100})),
            default_value: None,
            read_only: false,
            evidence: Evidence::DriverExposedNotTested,
        };
        assert!(number(Some(&setting), 350));
        assert!(!number(Some(&setting), 300));
        assert!(!number(Some(&setting), 700));
        setting.allowed = Some(json!([300, 600]));
        assert!(number(Some(&setting), 600));
        setting.read_only = true;
        assert!(!number(Some(&setting), 600));
        assert!(number(Some(&setting), 300));
        setting.read_only = false;
        setting.allowed = None;
        assert!(!number(Some(&setting), 300));
    }
}
