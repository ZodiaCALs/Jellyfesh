//! Forgiving deserialisers: Subsonic servers differ in small ways. Some send
//! ids as numbers, numbers as strings, or a single object where the schema
//! has an array (JSON converted from the original XML API).

use serde::Deserialize;
use serde::de::{DeserializeOwned, Deserializer};
use serde_json::Value;

/// A string, or a number written as one.
pub fn string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(opt_string(d)?.unwrap_or_default())
}

/// An optional string, or a number written as one; empty strings are none.
pub fn opt_string<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) if !s.is_empty() => Some(s),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

/// An optional non-negative whole number, also from a numeric string or a
/// float (rounded); anything else is none.
pub fn opt_u32<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    Ok(number(&Value::deserialize(d)?).map(|n| n.round().clamp(0.0, f64::from(u32::MAX)) as u32))
}

/// An optional number (milliseconds in lyrics may be fractional).
pub fn opt_f64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(number(&Value::deserialize(d)?))
}

/// An optional number that may be negative (a lyrics offset).
pub fn opt_signed_f64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(signed(&Value::deserialize(d)?))
}

/// An optional boolean, also from `"true"`/`"false"`.
pub fn opt_bool<'de, D: Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Bool(b) => Some(b),
        Value::String(s) if s.eq_ignore_ascii_case("true") => Some(true),
        Value::String(s) if s.eq_ignore_ascii_case("false") => Some(false),
        _ => None,
    })
}

/// A list of strings, also from a single string.
pub fn strings<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) if !s.is_empty() => vec![s],
        Value::Array(values) => values
            .into_iter()
            .filter_map(|v| match v {
                Value::String(s) if !s.is_empty() => Some(s),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    })
}

/// A list of objects, also from a single object; entries that do not parse
/// are skipped rather than failing the whole answer.
pub fn list<'de, D: Deserializer<'de>, T: DeserializeOwned>(d: D) -> Result<Vec<T>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Array(values) => values
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect(),
        value @ Value::Object(_) => serde_json::from_value(value).ok().into_iter().collect(),
        _ => Vec::new(),
    })
}

/// An optional object; anything else (or one that does not parse) is none.
pub fn opt_object<'de, D: Deserializer<'de>, T: DeserializeOwned>(d: D) -> Result<Option<T>, D::Error> {
    Ok(match Value::deserialize(d)? {
        value @ Value::Object(_) => serde_json::from_value(value).ok(),
        _ => None,
    })
}

fn number(value: &Value) -> Option<f64> {
    signed(value).filter(|n| *n >= 0.0)
}

fn signed(value: &Value) -> Option<f64> {
    let n = match value {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.trim().parse::<f64>().ok()?,
        _ => return None,
    };
    n.is_finite().then_some(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize, Debug, PartialEq)]
    struct Probe {
        #[serde(default, deserialize_with = "string")]
        id: String,
        #[serde(default, deserialize_with = "opt_u32")]
        n: Option<u32>,
        #[serde(default, deserialize_with = "opt_bool")]
        b: Option<bool>,
        #[serde(default, deserialize_with = "strings")]
        s: Vec<String>,
        #[serde(default, deserialize_with = "list")]
        l: Vec<Inner>,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct Inner {
        x: u32,
    }

    #[test]
    fn accepts_the_variants_servers_send() {
        let p: Probe =
            serde_json::from_str(r#"{"id": 42, "n": "7", "b": "true", "s": "USRC1", "l": {"x": 1}}"#)
                .unwrap();
        assert_eq!(p.id, "42");
        assert_eq!(p.n, Some(7));
        assert_eq!(p.b, Some(true));
        assert_eq!(p.s, vec!["USRC1"]);
        assert_eq!(p.l, vec![Inner { x: 1 }]);
        let p: Probe =
            serde_json::from_str(r#"{"id": "a", "n": 7.6, "s": ["A", ""], "l": [{"x": 1}, {"y": 2}]}"#)
                .unwrap();
        assert_eq!((p.n, p.b), (Some(8), None));
        assert_eq!(p.s, vec!["A"]);
        assert_eq!(p.l, vec![Inner { x: 1 }], "a broken entry is skipped");
        let p: Probe = serde_json::from_str(r#"{"n": -3, "l": null}"#).unwrap();
        assert_eq!((p.id.as_str(), p.n, p.l.len()), ("", None, 0));
    }
}
