use super::{
    ffi::{self, Api},
    options::{OptionAccess, Options},
    session::Session,
};
use scanner_types::{
    device::{Device, Evidence, Inspection, Setting, UnitSnapshot},
    error::Result,
};
use serde_json::json;
use std::path::Path;

pub(super) fn devices(api: &Api, library: &Path) -> Result<Vec<Device>> {
    let mut list = std::ptr::null();
    // SAFETY: the worker serializes all normal SANE operations. Copy the returned
    // list before any call can invalidate backend-owned storage.
    unsafe {
        ffi::check((api.devices)(&mut list, 0), "enumerate devices")?;
        if list.is_null() {
            return Err(ffi::error("Missing SANE device list."));
        }
        let mut result = Vec::new();
        for index in 0..=64 {
            let Some(device) = (*list.add(index)).as_ref() else {
                return Ok(result);
            };
            if index == 64 {
                return Err(ffi::error("Too many SANE devices."));
            }
            let name = ffi::string(device.name)?;
            if name.is_empty() || name.len() > 500 {
                return Err(ffi::error("Invalid SANE device name."));
            }
            result.push(Device {
                id: format!("sane:{name}"),
                name: format!(
                    "{} {}",
                    ffi::string(device.vendor)?,
                    ffi::string(device.model)?
                )
                .trim()
                .into(),
                driver_path: library.display().to_string(),
                driver_module_version: Some(format!(
                    "{}.{}.{}",
                    (api.version >> 24) & 255,
                    (api.version >> 16) & 255,
                    api.version & 65535
                )),
                extensions: [("saneType".into(), json!(ffi::string(device.kind)?))].into(),
                ..Device::default()
            });
        }
        unreachable!()
    }
}
pub(super) fn inspect(api: &Api, device: Device) -> Result<Inspection> {
    let session = Session::open(api, &device.id)?;
    let mut result = Inspection::new(device);
    result.session_opened = true;
    let options = Options {
        api,
        handle: session.handle,
    };
    let mut snapshot = UnitSnapshot {
        class_name: "SANE".into(),
        ..UnitSnapshot::default()
    };
    match options.list() {
        Err(error) => result.errors.push(error),
        Ok(descriptors) => {
            for option in descriptors {
                if !option.readable() {
                    continue;
                }
                match options.get(&option) {
                    Err(error) => result.errors.push(error),
                    Ok(value) => {
                        snapshot.settings.insert(
                            option.name.clone(),
                            Setting {
                                current: option.json(&value),
                                allowed: Some(option.allowed()),
                                default_value: None,
                                read_only: !option.writable(),
                                evidence: Evidence::DriverExposedNotTested,
                            },
                        );
                        snapshot.vendor_features.push(json!({"name":option.name,"title":option.title,"saneType":option.kind,"saneUnit":option.unit}));
                    }
                }
            }
        }
    }
    result.units.push(snapshot);
    // SANE has no ImageCapture functional-unit enum; do not invent one.
    drop(session);
    result.session_closed = true;
    Ok(result)
}
