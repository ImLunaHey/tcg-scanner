use serde_json::{Value, json};
#[derive(Clone)]
pub(crate) struct Field {
    pub required: bool,
    kind: Kind,
}
#[derive(Clone)]
enum Kind {
    Text {
        max: usize,
        uuid: bool,
        choices: Vec<&'static str>,
    },
    Integer(u64, u64),
    Boolean,
}
impl Field {
    pub fn text(max: usize) -> Self {
        Self {
            required: false,
            kind: Kind::Text {
                max,
                uuid: false,
                choices: vec![],
            },
        }
    }
    pub fn uuid() -> Self {
        Self {
            required: false,
            kind: Kind::Text {
                max: 36,
                uuid: true,
                choices: vec![],
            },
        }
    }
    pub fn choice(choices: &[&'static str]) -> Self {
        Self {
            required: false,
            kind: Kind::Text {
                max: 512,
                uuid: false,
                choices: choices.to_vec(),
            },
        }
    }
    pub fn integer(min: u64, max: u64) -> Self {
        Self {
            required: false,
            kind: Kind::Integer(min, max),
        }
    }
    pub fn boolean() -> Self {
        Self {
            required: false,
            kind: Kind::Boolean,
        }
    }
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
    pub fn schema(&self) -> Value {
        match &self.kind {
            Kind::Boolean => json!({"type":"boolean"}),
            Kind::Integer(min, max) => json!({"type":"integer","minimum":min,"maximum":max}),
            Kind::Text { max, uuid, choices } => {
                let mut value = json!({"type":"string","minLength":1,"maxLength":max});
                if *uuid {
                    value["format"] = json!("uuid");
                }
                if !choices.is_empty() {
                    value["enum"] = json!(choices);
                }
                value
            }
        }
    }
    pub fn valid(&self, value: &Value) -> bool {
        match &self.kind {
            Kind::Boolean => value.is_boolean(),
            Kind::Integer(min, max) => value.as_u64().is_some_and(|v| (*min..=*max).contains(&v)),
            Kind::Text { max, uuid, choices } => value.as_str().is_some_and(|v| {
                !v.is_empty()
                    && v.len() <= *max
                    && !v.contains('\0')
                    && (!uuid || uuid::Uuid::parse_str(v).is_ok())
                    && (choices.is_empty() || choices.contains(&v))
            }),
        }
    }
}
