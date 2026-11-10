use crate::Failure;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use uuid::Uuid;

pub(crate) struct Params(pub Map<String, Value>);
impl Params {
    pub fn new(value: Option<Value>) -> Result<Self, Failure> {
        match value {
            None => Ok(Self(Map::new())),
            Some(Value::Object(value)) => Ok(Self(value)),
            _ => Err(Failure::params("Parameters must be an object.")),
        }
    }
    pub fn only(&self, keys: &[&str]) -> Result<(), Failure> {
        if self.0.keys().any(|key| !keys.contains(&key.as_str())) {
            return Err(Failure::params("Unknown parameter."));
        }
        Ok(())
    }
    pub fn filter(&self, key: &str, max: usize) -> Result<&str, Failure> {
        match self.0.get(key) {
            None => Ok(""),
            Some(Value::String(s)) if s.len() <= max && !s.contains('\0') => Ok(s),
            _ => Err(Failure::params(format!(
                "{key} must be a string of at most {max} bytes."
            ))),
        }
    }
    pub fn optional_identifier(&self, key: &str) -> Result<Option<String>, Failure> {
        let Some(value) = self.0.get(key) else {
            return Ok(None);
        };
        match value.as_str() {
            Some(value) if !value.is_empty() && value.len() <= 512 && !value.contains('\0') => {
                Ok(Some(value.into()))
            }
            _ => Err(Failure::params(format!(
                "{key} must be a nonempty string of at most 512 bytes."
            ))),
        }
    }
    pub fn identifier(&self, key: &str) -> Result<String, Failure> {
        self.optional_identifier(key)?
            .ok_or_else(|| Failure::params(format!("Missing {key}.")))
    }
    pub fn uuid(&self, key: &str) -> Result<Uuid, Failure> {
        Uuid::parse_str(&self.identifier(key)?)
            .map_err(|_| Failure::params(format!("Expected {key} as UUID.")))
    }
    pub fn integer(&self, key: &str, fallback: u64, min: u64, max: u64) -> Result<u64, Failure> {
        let Some(value) = self.0.get(key) else {
            return Ok(fallback);
        };
        let number = value.as_u64().or_else(|| {
            value
                .as_f64()
                .filter(|v| {
                    v.is_finite() && v.fract() == 0.0 && *v >= 0.0 && *v <= 9_007_199_254_740_991.0
                })
                .map(|v| v as u64)
        });
        number
            .filter(|v| *v >= min && *v <= max)
            .ok_or_else(|| Failure::params(format!("{key} must be an integer within {min}–{max}.")))
    }
    pub fn decode<T: DeserializeOwned>(&self) -> Result<T, Failure> {
        serde_json::from_value(Value::Object(self.0.clone()))
            .map_err(|_| Failure::params("Invalid or missing parameter fields."))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_integral_json_numbers_but_not_booleans_fractions_or_overflow() {
        for (number, valid) in [
            (serde_json::json!(1.0), true),
            (serde_json::json!(true), false),
            (serde_json::json!(1.5), false),
            (serde_json::json!(-1), false),
            (serde_json::json!(u64::MAX), false),
        ] {
            let params = Params::new(Some(serde_json::json!({"limit":number}))).unwrap();
            assert_eq!(params.integer("limit", 50, 1, 100).is_ok(), valid);
        }
        assert!(Params::new(Some(serde_json::json!([]))).is_err());
    }
}
