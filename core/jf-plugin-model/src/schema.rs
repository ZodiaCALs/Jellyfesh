//! The settings schema profile: the subset of JSON Schema (draft 2020-12)
//! that the UI renders natively.
//!
//! A schema is one flat object of scalar properties:
//!
//! ```json
//! {
//!   "$schema": "https://json-schema.org/draft/2020-12/schema",
//!   "type": "object",
//!   "properties": {
//!     "server_url": { "type": "string", "format": "uri", "title": "Server address" },
//!     "max_bitrate": { "type": "integer", "minimum": 0, "maximum": 320, "default": 0 },
//!     "transcode": { "type": "boolean", "default": false },
//!     "quality": { "type": "string", "enum": ["low", "high"], "default": "high" }
//!   },
//!   "required": ["server_url"],
//!   "additionalProperties": false
//! }
//! ```
//!
//! * Top level: `$schema`, `$id`, `title`, `description`, `type` (must be
//!   `"object"`), `properties`, `required`, `additionalProperties` (must be
//!   `false`; it is implied).
//! * Properties (names: lowercase letters, digits and `_`, at most 32 of them):
//!   `type` (`string`, `integer`, `number`, `boolean`), `title`,
//!   `description`, `default`, `enum` (strings), `format` (`uri`: an absolute
//!   http or https URL; `email`), `minLength`, `maxLength`, `minimum`,
//!   `maximum`.
//! * Any other keyword is an error, so a plugin cannot rely on validation the
//!   host does not do. Secrets do not belong in settings: they are stored in
//!   plain JSON; logins go through `auth` flows.

use serde_json::{Map, Value};

/// Longest string value unless `maxLength` says less.
const MAX_STRING: u64 = 2048;
/// Most properties in a schema.
const MAX_PROPERTIES: usize = 32;
/// Largest settings object, serialised.
const MAX_SETTINGS_BYTES: usize = 64 * 1024;

/// A parsed, validated settings schema.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsSchema {
    raw: Value,
    /// The file as the package wrote it: `raw` with the properties in their order.
    text: String,
    properties: Vec<Property>,
    required: Vec<String>,
}

/// One setting.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// Key in the settings object.
    pub name: String,
    /// Value type.
    pub kind: PropertyKind,
    /// String format, if any.
    pub format: Option<Format>,
    /// Allowed values (strings only).
    pub choices: Option<Vec<String>>,
    /// Value used when the person has not set one.
    pub default: Option<Value>,
    min_length: Option<u64>,
    max_length: Option<u64>,
    minimum: Option<f64>,
    maximum: Option<f64>,
}

/// JSON type of a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyKind {
    /// A string.
    String,
    /// A whole number.
    Integer,
    /// Any number.
    Number,
    /// `true` or `false`.
    Boolean,
}

/// String formats the host checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// An absolute `http` or `https` URL with a host.
    Uri,
    /// Something with an `@` between two non-empty parts.
    Email,
}

impl SettingsSchema {
    /// The schema of a plugin without settings.
    pub fn empty() -> SettingsSchema {
        SettingsSchema {
            raw: Value::Object(Map::new()),
            text: "{}".to_owned(),
            properties: Vec::new(),
            required: Vec::new(),
        }
    }

    /// Parses a schema file.
    pub fn parse(text: &str) -> Result<SettingsSchema, String> {
        let raw: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
        let top = raw.as_object().ok_or("the schema must be a JSON object")?;
        for key in top.keys() {
            if !matches!(
                key.as_str(),
                "$schema"
                    | "$id"
                    | "title"
                    | "description"
                    | "type"
                    | "properties"
                    | "required"
                    | "additionalProperties"
            ) {
                return Err(format!("unsupported keyword {key:?} at the top level"));
            }
        }
        if top.get("type").and_then(Value::as_str) != Some("object") {
            return Err("the top-level type must be \"object\"".into());
        }
        if let Some(extra) = top.get("additionalProperties")
            && extra != &Value::Bool(false)
        {
            return Err("additionalProperties may only be false".into());
        }
        for key in ["title", "description", "$schema", "$id"] {
            if let Some(value) = top.get(key)
                && !value.is_string()
            {
                return Err(format!("{key} must be a string"));
            }
        }

        let mut properties = Vec::new();
        if let Some(props) = top.get("properties") {
            let props = props.as_object().ok_or("properties must be an object")?;
            if props.len() > MAX_PROPERTIES {
                return Err(format!("at most {MAX_PROPERTIES} properties"));
            }
            for (name, schema) in props {
                properties.push(parse_property(name, schema)?);
            }
        }

        let mut required = Vec::new();
        if let Some(list) = top.get("required") {
            let list = list.as_array().ok_or("required must be an array")?;
            for entry in list {
                let name = entry.as_str().ok_or("required must list property names")?;
                if !properties.iter().any(|p| p.name == name) {
                    return Err(format!("required names unknown property {name:?}"));
                }
                if !required.iter().any(|r| r == name) {
                    required.push(name.to_string());
                }
            }
        }

        Ok(SettingsSchema {
            raw,
            text: text.to_owned(),
            properties,
            required,
        })
    }

    /// The schema as JSON. Its maps are sorted by key; settings forms use [`Self::text`].
    pub fn json(&self) -> &Value {
        &self.raw
    }

    /// The schema as the package wrote it (checked by [`Self::parse`]), for settings forms: the
    /// same JSON as [`Self::json`], with the properties in the order the plugin lists them.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The properties.
    pub fn properties(&self) -> &[Property] {
        &self.properties
    }

    /// A property by name.
    pub fn property(&self, name: &str) -> Option<&Property> {
        self.properties.iter().find(|p| p.name == name)
    }

    /// An object with every default filled in.
    pub fn defaults(&self) -> Value {
        let mut out = Map::new();
        for p in &self.properties {
            if let Some(default) = &p.default {
                out.insert(p.name.clone(), default.clone());
            }
        }
        Value::Object(out)
    }

    /// Stored settings made to fit this schema (after a plugin update):
    /// values that still validate are kept, others fall back to their
    /// defaults, unknown keys are dropped; `required` is not enforced.
    pub fn fit(&self, stored: &Value) -> Value {
        let mut out = Map::new();
        for p in &self.properties {
            match stored.get(&p.name) {
                Some(value) if p.check(value).is_ok() => {
                    out.insert(p.name.clone(), value.clone());
                }
                _ => {
                    if let Some(default) = &p.default {
                        out.insert(p.name.clone(), default.clone());
                    }
                }
            }
        }
        Value::Object(out)
    }

    /// Checks settings the person entered and returns them with defaults
    /// filled in for missing properties.
    pub fn validate(&self, values: &Value) -> Result<Value, String> {
        let object = values.as_object().ok_or("settings must be a JSON object")?;
        let size = serde_json::to_vec(values).map(|v| v.len()).unwrap_or(usize::MAX);
        if size > MAX_SETTINGS_BYTES {
            return Err("settings are larger than 64 KiB".into());
        }
        for key in object.keys() {
            if self.property(key).is_none() {
                return Err(format!("unknown setting {key:?}"));
            }
        }
        let mut out = Map::new();
        for p in &self.properties {
            match object.get(&p.name) {
                Some(value) => {
                    p.check(value)?;
                    out.insert(p.name.clone(), value.clone());
                }
                None => {
                    if self.required.contains(&p.name) {
                        return Err(format!("{} is required", p.name));
                    }
                    if let Some(default) = &p.default {
                        out.insert(p.name.clone(), default.clone());
                    }
                }
            }
        }
        Ok(Value::Object(out))
    }
}

impl Property {
    fn check(&self, value: &Value) -> Result<(), String> {
        let name = &self.name;
        match self.kind {
            PropertyKind::Boolean => {
                value.as_bool().ok_or(format!("{name} must be true or false"))?;
            }
            PropertyKind::Integer => {
                let n = value.as_i64().ok_or(format!("{name} must be a whole number"))? as f64;
                self.check_range(n)?;
            }
            PropertyKind::Number => {
                let n = value.as_f64().ok_or(format!("{name} must be a number"))?;
                self.check_range(n)?;
            }
            PropertyKind::String => {
                let s = value.as_str().ok_or(format!("{name} must be text"))?;
                let len = s.chars().count() as u64;
                if len > self.max_length.unwrap_or(MAX_STRING) {
                    return Err(format!("{name} is too long"));
                }
                if len < self.min_length.unwrap_or(0) {
                    return Err(format!("{name} is too short"));
                }
                if s.chars().any(char::is_control) {
                    return Err(format!("{name} contains control characters"));
                }
                if let Some(choices) = &self.choices
                    && !choices.iter().any(|c| c == s)
                {
                    return Err(format!("{name} must be one of {}", choices.join(", ")));
                }
                match self.format {
                    Some(Format::Uri) if !s.is_empty() => {
                        check_uri(s).map_err(|e| format!("{name}: {e}"))?;
                    }
                    Some(Format::Email) if !s.is_empty() => {
                        let ok = s
                            .split_once('@')
                            .is_some_and(|(user, domain)| !user.is_empty() && domain.contains('.'));
                        if !ok {
                            return Err(format!("{name} must be an e-mail address"));
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn check_range(&self, n: f64) -> Result<(), String> {
        if !n.is_finite() {
            return Err(format!("{} must be finite", self.name));
        }
        if let Some(min) = self.minimum
            && n < min
        {
            return Err(format!("{} must be at least {min}", self.name));
        }
        if let Some(max) = self.maximum
            && n > max
        {
            return Err(format!("{} must be at most {max}", self.name));
        }
        Ok(())
    }
}

/// A `format: uri` value: an absolute http(s) URL with a host and no user
/// information.
pub(crate) fn check_uri(text: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(text).map_err(|e| format!("not a URL ({e})"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("only http and https URLs are allowed".into());
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err("the URL has no host".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("the URL must not contain a user name or password".into());
    }
    Ok(url)
}

fn parse_property(name: &str, schema: &Value) -> Result<Property, String> {
    if !crate::manifest::valid_setting_key(name) {
        return Err(format!(
            "property name {name:?} must be lowercase letters, digits and '_', starting with a letter"
        ));
    }
    let object = schema
        .as_object()
        .ok_or(format!("property {name:?} must be an object"))?;
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "type"
                | "title"
                | "description"
                | "default"
                | "enum"
                | "format"
                | "minLength"
                | "maxLength"
                | "minimum"
                | "maximum"
        ) {
            return Err(format!("unsupported keyword {key:?} in property {name:?}"));
        }
    }
    for key in ["title", "description"] {
        if let Some(v) = object.get(key)
            && !v.is_string()
        {
            return Err(format!("{name}.{key} must be a string"));
        }
    }
    let kind = match object.get("type").and_then(Value::as_str) {
        Some("string") => PropertyKind::String,
        Some("integer") => PropertyKind::Integer,
        Some("number") => PropertyKind::Number,
        Some("boolean") => PropertyKind::Boolean,
        _ => {
            return Err(format!(
                "property {name:?} needs a type: string, integer, number or boolean"
            ));
        }
    };
    let is_string = kind == PropertyKind::String;
    let is_number = matches!(kind, PropertyKind::Integer | PropertyKind::Number);
    let uint = |key: &str| -> Result<Option<u64>, String> {
        match object.get(key) {
            None => Ok(None),
            Some(_) if !is_string => Err(format!("{name}.{key} only applies to strings")),
            Some(v) => v
                .as_u64()
                .map(Some)
                .ok_or(format!("{name}.{key} must be a whole number")),
        }
    };
    let bound = |key: &str| -> Result<Option<f64>, String> {
        match object.get(key) {
            None => Ok(None),
            Some(_) if !is_number => Err(format!("{name}.{key} only applies to numbers")),
            Some(v) => v
                .as_f64()
                .map(Some)
                .ok_or(format!("{name}.{key} must be a number")),
        }
    };
    let format = match object.get("format") {
        None => None,
        Some(_) if !is_string => return Err(format!("{name}.format only applies to strings")),
        Some(Value::String(f)) if f == "uri" => Some(Format::Uri),
        Some(Value::String(f)) if f == "email" => Some(Format::Email),
        Some(other) => return Err(format!("{name}.format {other} is not supported (uri, email)")),
    };
    let choices = match object.get("enum") {
        None => None,
        Some(_) if !is_string => return Err(format!("{name}.enum only applies to strings")),
        Some(Value::Array(values)) if !values.is_empty() => Some(
            values
                .iter()
                .map(|v| v.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
                .ok_or(format!("{name}.enum must list strings"))?,
        ),
        Some(_) => return Err(format!("{name}.enum must be a non-empty array")),
    };
    let property = Property {
        name: name.to_string(),
        kind,
        format,
        choices,
        default: object.get("default").cloned(),
        min_length: uint("minLength")?,
        max_length: uint("maxLength")?,
        minimum: bound("minimum")?,
        maximum: bound("maximum")?,
    };
    if let Some(default) = &property.default {
        property
            .check(default)
            .map_err(|e| format!("default of {name}: {e}"))?;
    }
    Ok(property)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SCHEMA: &str = r#"{
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "title": "Server",
        "properties": {
            "server_url": { "type": "string", "format": "uri", "title": "Server address" },
            "max_bitrate": { "type": "integer", "minimum": 0, "maximum": 320, "default": 0 },
            "transcode": { "type": "boolean", "default": false },
            "quality": { "type": "string", "enum": ["low", "high"], "default": "high" },
            "volume": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
        },
        "required": ["server_url"],
        "additionalProperties": false
    }"#;

    #[test]
    fn forms_get_the_text_with_the_plugins_order() {
        let schema = SettingsSchema::parse(SCHEMA).unwrap();
        assert_eq!(schema.text(), SCHEMA);
        let at = |name: &str| schema.text().find(&format!("\"{name}\"")).unwrap();
        assert!(at("server_url") < at("max_bitrate") && at("max_bitrate") < at("transcode"));
        assert_eq!(SettingsSchema::empty().text(), "{}");
    }

    #[test]
    fn validates_and_fills_defaults() {
        let schema = SettingsSchema::parse(SCHEMA).unwrap();
        assert_eq!(schema.properties().len(), 5);
        let out = schema
            .validate(&json!({ "server_url": "http://192.168.1.2:4533" }))
            .unwrap();
        assert_eq!(
            out,
            json!({ "server_url": "http://192.168.1.2:4533", "max_bitrate": 0, "transcode": false, "quality": "high" })
        );
        assert_eq!(schema.defaults()["quality"], "high");
    }

    #[test]
    fn rejects_bad_values() {
        let schema = SettingsSchema::parse(SCHEMA).unwrap();
        for values in [
            json!([]),
            json!({}),
            json!({ "server_url": "ftp://x.example" }),
            json!({ "server_url": "not a url" }),
            json!({ "server_url": "https://user:pw@x.example" }),
            json!({ "server_url": "https://x.example", "max_bitrate": 400 }),
            json!({ "server_url": "https://x.example", "max_bitrate": 1.5 }),
            json!({ "server_url": "https://x.example", "transcode": "yes" }),
            json!({ "server_url": "https://x.example", "quality": "medium" }),
            json!({ "server_url": "https://x.example", "unknown": 1 }),
            json!({ "server_url": "https://x.example", "volume": 2.0 }),
        ] {
            assert!(schema.validate(&values).is_err(), "accepted {values}");
        }
    }

    #[test]
    fn rejects_unsupported_schemas() {
        for schema in [
            r#"{"type": "array"}"#,
            r#"{"type": "object", "additionalProperties": true}"#,
            r#"{"type": "object", "patternProperties": {}}"#,
            r#"{"type": "object", "properties": {"a": {"type": "object"}}}"#,
            r#"{"type": "object", "properties": {"a": {"type": "string", "pattern": "x"}}}"#,
            r#"{"type": "object", "properties": {"a": {"type": "string", "format": "date"}}}"#,
            r#"{"type": "object", "properties": {"a": {"type": "integer", "default": "x"}}}"#,
            r#"{"type": "object", "properties": {"A": {"type": "string"}}}"#,
            r#"{"type": "object", "properties": {"a": {"type": "boolean", "minLength": 1}}}"#,
            r#"{"type": "object", "required": ["missing"]}"#,
        ] {
            assert!(SettingsSchema::parse(schema).is_err(), "accepted {schema}");
        }
    }
}
