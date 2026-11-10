use crate::field::Field;
use scanner_types::error::{Result, ScannerError};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
#[derive(Clone)]
pub(crate) struct Tool {
    pub name: &'static str,
    pub method: &'static str,
    pub description: &'static str,
    pub fields: BTreeMap<&'static str, Field>,
    pub changes_state: bool,
    pub destructive: bool,
    pub defaults: Map<String, Value>,
}
impl Tool {
    pub fn new(
        name: &'static str,
        method: &'static str,
        description: &'static str,
        fields: Vec<(&'static str, Field)>,
    ) -> Self {
        Self {
            name,
            method,
            description,
            fields: fields.into_iter().collect(),
            changes_state: false,
            destructive: false,
            defaults: Map::new(),
        }
    }
    pub fn writes(mut self, destructive: bool) -> Self {
        self.changes_state = true;
        self.destructive = destructive;
        self
    }
    pub fn schema(&self) -> Value {
        let properties: Map<String, Value> = self
            .fields
            .iter()
            .map(|(name, field)| ((*name).into(), field.schema()))
            .collect();
        let required: Vec<_> = self
            .fields
            .iter()
            .filter(|(_, f)| f.required)
            .map(|(n, _)| n)
            .collect();
        json!({"name":self.name,"description":self.description,
            "inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},
            "annotations":{"readOnlyHint":!self.changes_state,"destructiveHint":self.destructive,"idempotentHint":true,"openWorldHint":false},
            "outputSchema":{"type":"object","required":["schemaVersion","ok"],"properties":{"schemaVersion":{"type":"integer","const":1},"ok":{"type":"boolean"},"data":{"type":"object"},"error":{"type":"object"}},"additionalProperties":false}})
    }
    pub fn parameters(&self, value: Value) -> Result<Value> {
        let bytes = serde_json::to_vec(&value).map_err(|_| invalid("Invalid arguments."))?;
        if bytes.len() > 60 * 1024 {
            return Err(invalid("Tool arguments exceed 60 KiB."));
        }
        let object = value
            .as_object()
            .ok_or_else(|| invalid("Tool arguments must be an object."))?;
        if object.keys().any(|k| !self.fields.contains_key(k.as_str())) {
            return Err(invalid("Unknown tool argument."));
        }
        for (name, field) in &self.fields {
            match object.get(*name) {
                Some(value) if !field.valid(value) => {
                    return Err(invalid(&format!(
                        "Invalid argument: {name}. See the tool schema."
                    )));
                }
                None if field.required => {
                    return Err(invalid(&format!("Missing required argument: {name}.")));
                }
                _ => {}
            }
        }
        let mut result = self.defaults.clone();
        result.extend(object.clone());
        Ok(Value::Object(result))
    }
}
fn invalid(message: &str) -> ScannerError {
    ScannerError::new("invalidParams", message)
}
pub(crate) fn result(value: Result<Value>) -> Value {
    let failed = value.is_err();
    let structured = match value {
        Ok(data) => json!({"schemaVersion":1,"ok":true,"data":data}),
        Err(error) => json!({"schemaVersion":1,"ok":false,"error":error}),
    };
    json!({"content":[{"type":"text","text":structured.to_string()}],"structuredContent":structured,"isError":failed})
}
