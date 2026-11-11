use crate::{
    camera_review::CameraSelection,
    error::{Result, ScannerError},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraImport {
    pub id: Uuid,
    pub selection: CameraSelection,
    pub base64: String,
    pub quantity: u32,
}
impl CameraImport {
    pub fn validate(&self) -> Result<()> {
        self.selection.validate()?;
        if self.id.is_nil() || !(1..=99).contains(&self.quantity) {
            return Err(ScannerError::new(
                "invalidParams",
                "Provide a capture ID and quantity from 1 to 99.",
            ));
        }
        Ok(())
    }
}
