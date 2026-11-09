use crate::Extensions;
use serde::{Deserialize, Serialize};

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
