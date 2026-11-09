use crate::Extensions;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Evidence {
    HardwareTested,
    DriverExposedNotTested,
    SoftwareImplemented,
    Unsupported,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    pub current: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
    pub read_only: bool,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitSnapshot {
    #[serde(rename = "type")]
    pub unit_type: u32,
    pub class_name: String,
    pub settings: std::collections::BTreeMap<String, Setting>,
    pub vendor_features: Vec<serde_json::Value>,
    pub templates: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    pub schema_version: u32,
    pub observed_at: chrono::DateTime<chrono::Utc>,
    pub device: Device,
    pub session_opened: bool,
    pub session_closed: bool,
    pub available_functional_unit_types: Vec<u32>,
    pub units: Vec<UnitSnapshot>,
    pub errors: Vec<crate::error::ScannerError>,
    pub warnings: Vec<String>,
    pub unknown_capabilities: Vec<String>,
    pub limitations: Vec<String>,
}

impl Inspection {
    pub fn new(device: Device) -> Self {
        Self {
            schema_version: 1,
            observed_at: chrono::Utc::now(),
            device,
            session_opened: false,
            session_closed: false,
            available_functional_unit_types: vec![],
            units: vec![],
            errors: vec![],
            warnings: vec![],
            unknown_capabilities: vec![],
            limitations: vec![
                "Property reads only; no scan was requested.".into(),
                "Current values are not defaults. Missing values remain unknown.".into(),
            ],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(default, rename = "usbVendorID")]
    pub usb_vendor_id: i32,
    #[serde(default, rename = "usbProductID")]
    pub usb_product_id: i32,
    #[serde(default)]
    pub driver_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver_module_version: Option<String>,
    #[serde(flatten)]
    pub extensions: Extensions,
}
