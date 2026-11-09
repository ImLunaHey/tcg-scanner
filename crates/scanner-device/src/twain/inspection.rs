use super::{capabilities, manager::Manager};
use scanner_types::{
    device::{Evidence, Inspection, Setting, UnitSnapshot},
    error::Result,
};

pub(super) fn inspect(manager: &mut Manager, id: &str) -> Result<Inspection> {
    let device = manager.open_source(id)?;
    let mut inspection = Inspection::new(device);
    inspection.session_opened = true;
    let mut unit = UnitSnapshot {
        class_name: "TWAIN".into(),
        ..UnitSnapshot::default()
    };
    for &(id, name) in capabilities::INSPECTION_CAPS {
        match capabilities::read(manager, id, 2) {
            Ok(value) => {
                let read_only = match capabilities::read(manager, id, 8) {
                    Ok(support) if support.kind == 5 => support.raw & 2 == 0,
                    _ => {
                        inspection
                            .unknown_capabilities
                            .push(format!("{name} write support"));
                        true
                    }
                };
                unit.settings.insert(
                    name.into(),
                    Setting {
                        current: value.json(),
                        allowed: None,
                        default_value: None,
                        read_only,
                        evidence: Evidence::DriverExposedNotTested,
                    },
                );
            }
            Err(error) if error.kind == "unsupported" => {
                inspection.unknown_capabilities.push(name.into())
            }
            Err(error) => inspection.errors.push(error),
        }
    }
    inspection.units.push(unit);
    match manager.close_source() {
        Ok(()) => inspection.session_closed = true,
        Err(error) => inspection.errors.push(error),
    }
    Ok(inspection)
}
