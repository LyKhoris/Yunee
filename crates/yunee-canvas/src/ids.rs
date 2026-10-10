//! Tolerant parsing for Canvas ids.
//!
//! Canvas returns ids as numbers by default, but sends them as strings when the
//! client asks with `Accept: application/json+canvas-string-ids`. Rather than
//! trust the header (older/self-hosted installs may ignore it), every id field
//! is parsed through here and normalized to a `String`.

use serde::Deserialize;
use serde::de::{self, Deserializer};

fn value_to_id(value: serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) if !s.is_empty() => Some(s),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Required id: accepts a number or a string.
pub fn id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    value_to_id(value).ok_or_else(|| de::Error::custom("expected a Canvas id (number or string)"))
}

/// Optional id: also accepts `null`.
pub fn opt_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(value_to_id))
}

/// Optional id sequence.
pub fn opt_id_vec<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    let value = Option::<Vec<serde_json::Value>>::deserialize(deserializer)?;
    Ok(value
        .unwrap_or_default()
        .into_iter()
        .filter_map(value_to_id)
        .collect())
}

/// Optional number, number-or-string, or null.
pub fn opt_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(value_to_f64))
}

/// Optional integer, number-or-string, or null. Fractions truncate.
pub fn opt_int<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i64>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|v| match v {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        serde_json::Value::String(s) => s.trim().parse::<i64>().ok(),
        serde_json::Value::Bool(b) => Some(b as i64),
        _ => None,
    }))
}

fn value_to_f64(value: serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Holder {
        #[serde(deserialize_with = "id")]
        id: String,
        #[serde(default, deserialize_with = "opt_id")]
        parent: Option<String>,
    }

    #[test]
    fn accepts_numbers_and_strings() {
        let a: Holder = serde_json::from_str(r#"{"id": 42, "parent": null}"#).unwrap();
        assert_eq!(a.id, "42");
        assert_eq!(a.parent, None);
        let b: Holder = serde_json::from_str(r#"{"id": "42", "parent": 7}"#).unwrap();
        assert_eq!(b.id, "42");
        assert_eq!(b.parent.as_deref(), Some("7"));
    }
}
