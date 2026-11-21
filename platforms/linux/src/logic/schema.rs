//! Plugin settings forms from the plugin's JSON Schema (ADR-0004: a subset
//! of draft 2020-12, rendered natively).
//!
//! Supported: an object whose `properties` are strings (with `format`
//! `password` or `uri`, or `writeOnly` for secrets), booleans, integers and
//! numbers (`minimum`, `maximum`, `multipleOf`), and `enum`s (labels from
//! `enumNames`, or `oneOf` entries with `const` and `title`). Schema
//! profile 1.2 (docs/plugins/authoring.md) adds `"format": "color"`
//! (`#RRGGBB`, a colour well), sliders (a number with `minimum` and
//! `maximum`, `multipleOf` its step), ordered checklists (an array of
//! unique strings from an `enum` or `oneOf`: the value lists the shown
//! items in order) and `x-jf-group` headings. Unknown property types are
//! skipped. Properties keep the schema's order.

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
    /// `#RRGGBB`, a colour well.
    Color,
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
    /// A number with a `minimum` and a `maximum`: a slider.
    Slider {
        min: f64,
        max: f64,
        step: f64,
    },
    /// `(value, label)` pairs.
    Choice {
        options: Vec<(Value, String)>,
    },
    /// An ordered checklist of `(value, label)` items; the setting lists the
    /// shown ones in order.
    Checklist {
        options: Vec<(String, String)>,
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
    /// The heading it shows under (`x-jf-group`).
    pub group: Option<String>,
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
            group: prop
                .get("x-jf-group")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|g| !g.is_empty())
                .map(str::to_owned),
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
            if format == "color" {
                return Some(FieldKind::Color);
            }
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
        "number" => match (num("minimum"), num("maximum")) {
            (Some(min), Some(max)) if min < max => Some(FieldKind::Slider {
                min,
                max,
                step: num("multipleOf")
                    .filter(|s| *s > 0.0)
                    .unwrap_or((max - min) / 100.0),
            }),
            (min, max) => Some(FieldKind::Number {
                min: min.unwrap_or(-1_000_000.0),
                max: max.unwrap_or(1_000_000.0),
                step: num("multipleOf").unwrap_or(0.1),
            }),
        },
        "array" => checklist(prop).map(|options| FieldKind::Checklist { options }),
        _ => None,
    }
}

/// The items of an ordered checklist: `"type": "array"` of unique strings
/// from an `enum` or `oneOf`.
fn checklist(prop: &Value) -> Option<Vec<(String, String)>> {
    if !prop.get("uniqueItems").and_then(Value::as_bool).unwrap_or(false) {
        return None;
    }
    let items = choices(prop.get("items")?)?;
    let options: Vec<(String, String)> = items
        .into_iter()
        .filter_map(|(value, label)| Some((value.as_str()?.to_owned(), label)))
        .collect();
    (!options.is_empty()).then_some(options)
}

/// The fields under their headings: consecutive fields with the same
/// `x-jf-group` together, in the schema's order.
pub fn grouped(fields: Vec<Field>) -> Vec<(Option<String>, Vec<Field>)> {
    let mut out: Vec<(Option<String>, Vec<Field>)> = Vec::new();
    for field in fields {
        match out.last_mut() {
            Some((group, list)) if *group == field.group => list.push(field),
            _ => out.push((field.group.clone(), vec![field])),
        }
    }
    out
}

/// A colour as `#RRGGBB`, from what was typed (`#rgb`, `RRGGBB` and case
/// are forgiven); `None` when it is not a colour.
pub fn color(text: &str) -> Option<String> {
    let hex = text.trim().trim_start_matches('#');
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let full = match hex.len() {
        6 => hex.to_owned(),
        3 => hex.chars().flat_map(|c| [c, c]).collect(),
        _ => return None,
    };
    Some(format!("#{}", full.to_ascii_uppercase()))
}

/// `#RRGGBB` as red, green and blue.
pub fn rgb(color_text: &str) -> Option<[u8; 3]> {
    let c = color(color_text)?;
    let byte = |i: usize| u8::from_str_radix(&c[i..i + 2], 16).ok();
    Some([byte(1)?, byte(3)?, byte(5)?])
}

/// A checklist's rows: the shown items in the setting's order, then the
/// others in the schema's, each with whether it shows.
pub fn checklist_rows(options: &[(String, String)], value: Option<&Value>) -> Vec<(String, String, bool)> {
    let shown: Vec<&str> = value
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let label = |v: &str| options.iter().find(|(o, _)| o == v).map(|(_, l)| l.clone());
    let mut rows: Vec<(String, String, bool)> = Vec::new();
    for v in shown {
        if let Some(l) = label(v)
            && !rows.iter().any(|(r, _, _)| r == v)
        {
            rows.push((v.to_owned(), l, true));
        }
    }
    for (v, l) in options {
        if !rows.iter().any(|(r, _, _)| r == v) {
            rows.push((v.clone(), l.clone(), false));
        }
    }
    rows
}

/// A checklist's setting: the shown items, in the rows' order.
pub fn checklist_value(rows: &[(String, String, bool)]) -> Value {
    Value::Array(
        rows.iter()
            .filter(|(_, _, shown)| *shown)
            .map(|(v, _, _)| Value::String(v.clone()))
            .collect(),
    )
}

/// The language tag the core's `localized_settings_schema` takes for the
/// app's locale (`LANGUAGE`, `LC_ALL`, `LC_MESSAGES` or `LANG`), for the
/// languages the app is translated into; `None` for English and the rest.
pub fn schema_language(locale: &str) -> Option<&'static str> {
    let first = locale.split(':').find(|l| !l.trim().is_empty())?.trim();
    let base = first.split(['.', '@']).next().unwrap_or(first);
    let mut parts = base.split(['_', '-']);
    let language = parts.next()?.to_ascii_lowercase();
    let region = parts.next().map(str::to_ascii_uppercase);
    match (language.as_str(), region.as_deref()) {
        ("ja", _) => Some("ja-JP"),
        ("zh", Some("TW" | "HK" | "MO" | "HANT")) => Some("zh-TW"),
        ("zh", _) => Some("zh-Hans"),
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
            "gain": { "type": "number", "minimum": -6 },
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
                "volume",
                "gain"
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
        assert_eq!(
            f[6].kind,
            FieldKind::Slider {
                min: 0.0,
                max: 1.0,
                step: 0.01
            }
        );
        assert!(matches!(f[7].kind, FieldKind::Number { min, .. } if min == -6.0));
        assert!(f.iter().all(|f| f.group.is_none()));
    }

    const PROFILE_1_2: &str = r##"{
        "type": "object",
        "properties": {
            "corners": { "type": "number", "minimum": 0, "maximum": 2, "multipleOf": 0.05, "x-jf-group": "Look" },
            "typeface": { "type": "string", "oneOf": [ { "const": "system", "title": "System" } ], "x-jf-group": "Look" },
            "colour": { "type": "string", "format": "color", "default": "#FF8A3D", "x-jf-group": "Lyrics" },
            "sidebar": {
                "type": "array",
                "items": { "type": "string", "oneOf": [
                    { "const": "home", "title": "Home" },
                    { "const": "albums", "title": "Albums" },
                    { "const": "songs", "title": "Songs" }
                ] },
                "uniqueItems": true,
                "x-jf-group": "Layout"
            },
            "tags": { "type": "array", "items": { "type": "string", "enum": ["a"] } },
            "loose": { "type": "boolean" }
        }
    }"##;

    #[test]
    fn profile_1_2_fields() {
        let f = fields(PROFILE_1_2).unwrap();
        let keys: Vec<&str> = f.iter().map(|f| f.key.as_str()).collect();
        // An array without uniqueItems is no checklist: skipped.
        assert_eq!(keys, ["corners", "typeface", "colour", "sidebar", "loose"]);
        assert_eq!(
            f[0].kind,
            FieldKind::Slider {
                min: 0.0,
                max: 2.0,
                step: 0.05
            }
        );
        assert_eq!(f[2].kind, FieldKind::Color);
        assert_eq!(
            f[3].kind,
            FieldKind::Checklist {
                options: vec![
                    ("home".into(), "Home".into()),
                    ("albums".into(), "Albums".into()),
                    ("songs".into(), "Songs".into())
                ]
            }
        );
        let grouped_fields = grouped(f);
        let groups: Vec<(Option<&str>, usize)> = grouped_fields
            .iter()
            .map(|(g, list)| (g.as_deref(), list.len()))
            .collect();
        assert_eq!(
            groups,
            [
                (Some("Look"), 2),
                (Some("Lyrics"), 1),
                (Some("Layout"), 1),
                (None, 1)
            ]
        );
    }

    #[test]
    fn colours_as_typed() {
        assert_eq!(color("#ff8a3d").as_deref(), Some("#FF8A3D"));
        assert_eq!(color(" FF8A3D ").as_deref(), Some("#FF8A3D"));
        assert_eq!(color("#f80").as_deref(), Some("#FF8800"));
        assert_eq!(color("#ff8a3"), None);
        assert_eq!(color("orange"), None);
        assert_eq!(rgb("#FF8A3D"), Some([0xFF, 0x8A, 0x3D]));
        assert_eq!(rgb("nope"), None);
    }

    #[test]
    fn checklists_keep_the_shown_order() {
        let options: Vec<(String, String)> = [("home", "Home"), ("albums", "Albums"), ("songs", "Songs")]
            .iter()
            .map(|(v, l)| ((*v).to_owned(), (*l).to_owned()))
            .collect();
        let rows = checklist_rows(&options, Some(&json!(["songs", "home", "gone", "home"])));
        let order: Vec<(&str, bool)> = rows.iter().map(|(v, _, s)| (v.as_str(), *s)).collect();
        assert_eq!(order, [("songs", true), ("home", true), ("albums", false)]);
        assert_eq!(checklist_value(&rows), json!(["songs", "home"]));
        // Nothing set: every item, none shown.
        let rows = checklist_rows(&options, None);
        assert!(rows.iter().all(|(_, _, s)| !s));
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn languages_for_localised_schemas() {
        assert_eq!(schema_language("ja_JP.UTF-8"), Some("ja-JP"));
        assert_eq!(schema_language("zh_CN.UTF-8"), Some("zh-Hans"));
        assert_eq!(schema_language("zh_TW.UTF-8"), Some("zh-TW"));
        assert_eq!(schema_language("zh_HK"), Some("zh-TW"));
        assert_eq!(schema_language(":ja:en"), Some("ja-JP"));
        assert_eq!(schema_language("en_GB.UTF-8"), None);
        assert_eq!(schema_language("C.UTF-8"), None);
        assert_eq!(schema_language(""), None);
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
