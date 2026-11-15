//! Plugin settings forms from the plugin's JSON Schema (ADR-0004: a subset
//! of draft 2020-12, rendered natively).
//!
//! Supported: an object whose `properties` are strings (with `format`
//! `password` or `uri`, or `writeOnly` for secrets), booleans, integers and
//! numbers (`minimum`, `maximum`, `multipleOf`), and `enum`s (labels from
//! `enumNames`, or `oneOf` entries with `const` and `title`). Unknown
//! property types are skipped. Properties keep the schema's order.

use std::fmt;

use serde::Deserialize;
use serde::de::{Deserializer, MapAccess, Visitor};
use serde_json::{Map, Value};

/// How a setting is edited.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldKind {
    Text {
        secret: bool,
        url: bool,
    },
    Toggle,
    Integer {
        min: f64,
        max: f64,
        step: f64,
    },
    Number {
        min: f64,
        max: f64,
        step: f64,
    },
    /// `(value, label)` pairs.
    Choice {
        options: Vec<(Value, String)>,
    },
}

/// One setting.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    pub kind: FieldKind,
    pub required: bool,
    pub default: Option<Value>,
}

/// Properties in document order.
#[derive(Debug, Default)]
struct Ordered(Vec<(String, Value)>);

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Ordered;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map of properties")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
                let mut out = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, Value>()? {
                    out.push((k, v));
                }
                Ok(Ordered(out))
            }
        }
        deserializer.deserialize_map(V)
    }
}

#[derive(Debug, Default, Deserialize)]
struct Schema {
    #[serde(default)]
    properties: Ordered,
    #[serde(default)]
    required: Vec<String>,
}

/// The fields of a settings schema. An empty schema gives no fields.
pub fn fields(schema_json: &str) -> Result<Vec<Field>, String> {
    if schema_json.trim().is_empty() {
        return Ok(Vec::new());
    }
    let schema: Schema = serde_json::from_str(schema_json).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (key, prop) in schema.properties.0 {
        let Some(kind) = kind_of(&prop) else { continue };
        let title = prop
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| humanize(&key));
        out.push(Field {
            required: schema.required.contains(&key),
            description: prop.get("description").and_then(Value::as_str).map(str::to_owned),
            default: prop.get("default").cloned(),
            key,
            title,
            kind,
        });
    }
    Ok(out)
}

fn kind_of(prop: &Value) -> Option<FieldKind> {
    if let Some(options) = choices(prop) {
        return Some(FieldKind::Choice { options });
    }
    let num = |k: &str| prop.get(k).and_then(Value::as_f64);
    match prop.get("type").and_then(Value::as_str)? {
        "string" => {
            let format = prop.get("format").and_then(Value::as_str).unwrap_or_default();
            let write_only = prop.get("writeOnly").and_then(Value::as_bool).unwrap_or(false);
            Some(FieldKind::Text {
                secret: write_only || format == "password",
                url: format == "uri" || format == "url",
            })
        }
        "boolean" => Some(FieldKind::Toggle),
        "integer" => Some(FieldKind::Integer {
            min: num("minimum").unwrap_or(0.0),
            max: num("maximum").unwrap_or(1_000_000.0),
            step: num("multipleOf").unwrap_or(1.0).max(1.0),
        }),
        "number" => Some(FieldKind::Number {
            min: num("minimum").unwrap_or(-1_000_000.0),
            max: num("maximum").unwrap_or(1_000_000.0),
            step: num("multipleOf").unwrap_or(0.1),
        }),
        _ => None,
    }
}

fn choices(prop: &Value) -> Option<Vec<(Value, String)>> {
    if let Some(values) = prop.get("enum").and_then(Value::as_array) {
        let names = prop.get("enumNames").and_then(Value::as_array);
        return Some(
            values
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let label = names
                        .and_then(|n| n.get(i))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .unwrap_or_else(|| label_of(v));
                    (v.clone(), label)
                })
                .collect(),
        );
    }
    let one_of = prop.get("oneOf").and_then(Value::as_array)?;
    let options: Vec<(Value, String)> = one_of
        .iter()
        .filter_map(|o| {
            let value = o.get("const")?.clone();
            let label = o
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| label_of(&value));
            Some((value, label))
        })
        .collect();
    (!options.is_empty()).then_some(options)
}

fn label_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// `server_url` → `Server url`.
/// A label for a property without a `title`, in Title Case like every row
/// title (docs/ui.md, "Words"): `api_key` → `Api Key`.
fn humanize(key: &str) -> String {
    key.split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The current value of `field` in a plugin's settings JSON, else its default.
pub fn current(settings_json: &str, field: &Field) -> Option<Value> {
    serde_json::from_str::<Map<String, Value>>(settings_json)
        .ok()
        .and_then(|m| m.get(&field.key).cloned())
        .filter(|v| !v.is_null())
        .or_else(|| field.default.clone())
}

/// The settings JSON with `key` set to `value` (other keys kept).
pub fn with_value(settings_json: &str, key: &str, value: Value) -> String {
    let mut map = serde_json::from_str::<Map<String, Value>>(settings_json).unwrap_or_default();
    map.insert(key.to_owned(), value);
    Value::Object(map).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SCHEMA: &str = r#"{
        "type": "object",
        "required": ["server_url"],
        "properties": {
            "server_url": { "type": "string", "format": "uri", "title": "Server" },
            "api_key": { "type": "string", "writeOnly": true },
            "transcode": { "type": "boolean", "default": false },
            "max_bitrate": { "type": "integer", "minimum": 64, "maximum": 320, "multipleOf": 32 },
            "quality": { "type": "string", "enum": ["low", "high"], "enumNames": ["Low", "High"] },
            "region": { "oneOf": [ { "const": "eu", "title": "Europe" }, { "const": "us" } ] },
            "volume": { "type": "number", "minimum": 0, "maximum": 1 },
            "nested": { "type": "object" }
        }
    }"#;

    #[test]
    fn reads_fields_in_order() {
        let f = fields(SCHEMA).unwrap();
        let keys: Vec<&str> = f.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "server_url",
                "api_key",
                "transcode",
                "max_bitrate",
                "quality",
                "region",
                "volume"
            ]
        );
        assert_eq!(f[0].title, "Server");
        assert!(f[0].required);
        assert_eq!(
            f[0].kind,
            FieldKind::Text {
                secret: false,
                url: true
            }
        );
        assert_eq!(f[1].title, "Api Key");
        assert_eq!(
            f[1].kind,
            FieldKind::Text {
                secret: true,
                url: false
            }
        );
        assert_eq!(f[2].kind, FieldKind::Toggle);
        assert_eq!(
            f[3].kind,
            FieldKind::Integer {
                min: 64.0,
                max: 320.0,
                step: 32.0
            }
        );
        assert_eq!(
            f[4].kind,
            FieldKind::Choice {
                options: vec![(json!("low"), "Low".into()), (json!("high"), "High".into())]
            }
        );
        assert_eq!(
            f[5].kind,
            FieldKind::Choice {
                options: vec![(json!("eu"), "Europe".into()), (json!("us"), "us".into())]
            }
        );
        assert!(matches!(f[6].kind, FieldKind::Number { .. }));
    }

    #[test]
    fn untitled_properties_get_title_case_labels() {
        assert_eq!(humanize("server_url"), "Server Url");
        assert_eq!(humanize("max-bitrate"), "Max Bitrate");
        assert_eq!(humanize("user__name"), "User Name");
        assert_eq!(humanize(""), "");
    }

    #[test]
    fn empty_and_broken_schemas() {
        assert!(fields("").unwrap().is_empty());
        assert!(fields("{}").unwrap().is_empty());
        assert!(fields("not json").is_err());
    }

    #[test]
    fn values_and_edits() {
        let f = fields(SCHEMA).unwrap();
        let settings = r#"{"server_url":"https://music.example","transcode":null}"#;
        assert_eq!(current(settings, &f[0]), Some(json!("https://music.example")));
        assert_eq!(
            current(settings, &f[2]),
            Some(json!(false)),
            "null falls back to the default"
        );
        assert_eq!(current("", &f[3]), None);
        let edited = with_value(settings, "transcode", json!(true));
        let v: Value = serde_json::from_str(&edited).unwrap();
        assert_eq!(v["transcode"], json!(true));
        assert_eq!(v["server_url"], json!("https://music.example"));
        assert_eq!(with_value("garbage", "a", json!(1)), r#"{"a":1}"#);
    }
}
