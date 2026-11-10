use crate::error::{Error, Result};
use std::collections::BTreeMap;

pub(crate) struct Path {
    pub parts: Vec<String>,
    pub query: BTreeMap<String, String>,
}
impl Path {
    pub fn parse(target: &str) -> Result<Self> {
        if target.len() > 8192
            || !target.starts_with('/')
            || target.starts_with("//")
            || target.contains('#')
        {
            return Err(Error::params("Invalid request target."));
        }
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let mut parts = vec![];
        if path != "/" {
            for part in path[1..].split('/') {
                let value = decode(part)?;
                if value.is_empty()
                    || matches!(value.as_str(), "." | "..")
                    || value.contains(['/', '\\', '\0'])
                    || value.len() > 512
                {
                    return Err(Error::params("Invalid path component."));
                }
                parts.push(value);
            }
        }
        let mut values = BTreeMap::new();
        if !query.is_empty() {
            for item in query.split('&') {
                let (key, value) = item
                    .split_once('=')
                    .ok_or_else(|| Error::params("Query parameters require values."))?;
                let (key, value) = (decode(key)?, decode(value)?);
                if key.is_empty() || values.insert(key, value).is_some() {
                    return Err(Error::params("Invalid or duplicate query parameter."));
                }
            }
        }
        Ok(Self {
            parts,
            query: values,
        })
    }
    pub fn only(&self, keys: &[&str]) -> Result<()> {
        if self.query.keys().any(|k| !keys.contains(&k.as_str())) {
            Err(Error::params("Unknown query parameter."))
        } else {
            Ok(())
        }
    }
    pub fn number(&self, key: &str, fallback: u64, min: u64, max: u64) -> Result<u64> {
        let Some(value) = self.query.get(key) else {
            return Ok(fallback);
        };
        value
            .parse::<u64>()
            .ok()
            .filter(|n| *n >= min && *n <= max)
            .ok_or_else(|| Error::params("Invalid numeric query parameter."))
    }
}
fn decode(input: &str) -> Result<String> {
    let mut result = Vec::new();
    let mut bytes = input.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let hi = bytes
                .next()
                .and_then(hex)
                .ok_or_else(|| Error::params("Invalid percent escape."))?;
            let lo = bytes
                .next()
                .and_then(hex)
                .ok_or_else(|| Error::params("Invalid percent escape."))?;
            result.push((hi << 4) | lo);
        } else {
            result.push(b);
        }
    }
    let result =
        String::from_utf8(result).map_err(|_| Error::params("Expected UTF-8 request target."))?;
    if result.chars().any(char::is_control) {
        return Err(Error::params("Control characters are not allowed."));
    }
    Ok(result)
}
fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_ambiguous_targets_and_decodes_unicode_once() {
        for target in [
            "//host/path",
            "/a//b",
            "/a/..",
            "/a/%2f",
            "/a/%5c",
            "/a/%00",
            "/a/%ff",
            "/a/%z1",
            "/a?x=1&x=2",
            "/a?x",
            "/a#fragment",
        ] {
            assert!(Path::parse(target).is_err(), "{target}");
        }
        let path = Path::parse("/api/v1/profiles/Caf%C3%A9?search=a+b").unwrap();
        assert_eq!(path.parts.last().unwrap(), "Café");
        assert_eq!(path.query["search"], "a+b");
    }
}
