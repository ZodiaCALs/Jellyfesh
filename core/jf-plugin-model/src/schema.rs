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
//!
//! # Profile 1.2
//!
//! Packages with `api_version = "1.2"` or later ([`Profile::V1_2`], ADR-0020)
//! may also use:
//!
//! * `"format": "color"`: a string `#RRGGBB`, shown as a colour well;
//! * `multipleOf` on a `number` or `integer` with both `minimum` and
//!   `maximum` (a slider; `multipleOf` is its step; any number with both
//!   bounds is a slider);
//! * `"type": "array"` with `"items": { "type": "string", "enum": [...] }`
//!   (or `oneOf`) and `"uniqueItems": true`: an ordered checklist whose value
//!   lists the shown items in order;
//! * `"oneOf": [{ "const": "stage", "title": "Stage" }, ...]` on a string (or
//!   in `items`): choices with titles, instead of `enum`;
//! * `"x-jf-group": "Lyrics"` on a property: consecutive properties with the
//!   same group are shown together under that heading.
//!
//! Older packages that use them are refused. Translations of the titles live
//! in the package (`crate::l10n`).

use serde_json::{Map, Value};

/// Longest string value unless `maxLength` says less.
const MAX_STRING: u64 = 2048;
/// Most properties in a schema.
const MAX_PROPERTIES: usize = 32;
/// Largest settings object, serialised.
const MAX_SETTINGS_BYTES: usize = 64 * 1024;

/// The keywords a package may use: the profile of its `api_version`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Profile {
    /// API 1.0 and 1.1: flat scalars, `enum`, `uri` and `email`.
    V1_1,
    /// API 1.2: also colours, slider steps, ordered checklists, `oneOf`
    /// choices with titles and groups (ADR-0020).
    V1_2,
}

impl Profile {
    /// The profile of a package's `api_version`.
    pub fn for_api(api: crate::manifest::ApiVersion) -> Profile {
        if api.major > 1 || api.minor >= 2 {
            Profile::V1_2
        } else {
            Profile::V1_1
        }
    }
}

/// A parsed, validated settings schema.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsSchema {
    raw: Value,
    /// The file as the package wrote it: `raw` with the properties in their order.
    text: String,
    properties: Vec<Property>,
    required: Vec<String>,
}

/// A choice with its title (`oneOf` in profile 1.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    /// The value stored.
    pub value: String,
    /// What the person sees; the value itself for `enum` choices.
    pub title: Option<String>,
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
    /// Allowed values (strings, and the items of an array).
    pub choices: Option<Vec<String>>,
    /// The titles of the choices, when the schema gives them (`oneOf`).
    pub choice_titles: Vec<Choice>,
    /// Value used when the person has not set one.
    pub default: Option<Value>,
    /// The heading it is shown under (`x-jf-group`, profile 1.2).
    pub group: Option<String>,
    /// A slider's step (`multipleOf`, profile 1.2).
    pub step: Option<f64>,
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
    /// An ordered list of distinct choices (profile 1.2).
    Array,
}

/// String formats the host checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// An absolute `http` or `https` URL with a host.
    Uri,
    /// Something with an `@` between two non-empty parts.
    Email,
    /// `#RRGGBB` (profile 1.2).
    Color,
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

    /// Parses a schema file in profile 1.1 (packages for API 1.0 and 1.1).
    pub fn parse(text: &str) -> Result<SettingsSchema, String> {
        SettingsSchema::parse_with(text, Profile::V1_1)
    }

    /// Parses a schema file in `profile`.
    pub fn parse_with(text: &str, profile: Profile) -> Result<SettingsSchema, String> {
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
                properties.push(parse_property(name, schema, profile)?);
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
            PropertyKind::Array => {
                let list = value.as_array().ok_or(format!("{name} must be a list"))?;
                let choices = self.choices.as_deref().unwrap_or_default();
                let mut seen: Vec<&str> = Vec::new();
                for entry in list {
                    let entry = entry.as_str().ok_or(format!("{name} must list text"))?;
                    if !choices.iter().any(|c| c == entry) {
                        return Err(format!("{name} cannot list {entry:?}"));
                    }
                    if seen.contains(&entry) {
                        return Err(format!("{name} lists {entry:?} twice"));
                    }
                    seen.push(entry);
                }
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
                    Some(Format::Color) if !is_color(s) => {
                        return Err(format!("{name} must be a colour like #RRGGBB"));
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
        if let Some(step) = self.step {
            let steps = n / step;
            if (steps - steps.round()).abs() > 1e-9 * steps.abs().max(1.0) {
                return Err(format!("{} must be a multiple of {step}", self.name));
            }
        }
        Ok(())
    }

    /// The smallest value, for numbers.
    pub fn minimum(&self) -> Option<f64> {
        self.minimum
    }

    /// The largest value, for numbers.
    pub fn maximum(&self) -> Option<f64> {
        self.maximum
    }

    /// Whether the apps show it as a slider (a number with both bounds).
    pub fn is_slider(&self) -> bool {
        self.minimum.is_some() && self.maximum.is_some()
    }
}

/// `#RRGGBB`.
pub fn is_color(text: &str) -> bool {
    text.len() == 7 && text.starts_with('#') && text[1..].bytes().all(|b| b.is_ascii_hexdigit())
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

fn parse_property(name: &str, schema: &Value, profile: Profile) -> Result<Property, String> {
    if !crate::manifest::valid_setting_key(name) {
        return Err(format!(
            "property name {name:?} must be lowercase letters, digits and '_', starting with a letter"
        ));
    }
    let object = schema
        .as_object()
        .ok_or(format!("property {name:?} must be an object"))?;
    let v12 = profile >= Profile::V1_2;
    for key in object.keys() {
        let known = matches!(
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
        ) || (v12
            && matches!(
                key.as_str(),
                "multipleOf" | "items" | "uniqueItems" | "oneOf" | "x-jf-group"
            ));
        if !known {
            let later = matches!(
                key.as_str(),
                "multipleOf" | "items" | "uniqueItems" | "oneOf" | "x-jf-group"
            );
            return Err(if later {
                format!("{key:?} in property {name:?} needs api_version 1.2 (settings schema profile 1.2)")
            } else {
                format!("unsupported keyword {key:?} in property {name:?}")
            });
        }
    }
    for key in ["title", "description", "x-jf-group"] {
        if let Some(v) = object.get(key)
            && !v.is_string()
        {
            return Err(format!("{name}.{key} must be a string"));
        }
    }
    let group = match object.get("x-jf-group").and_then(Value::as_str) {
        Some(g) if g.trim().is_empty() || g.len() > 64 || g.chars().any(char::is_control) => {
            return Err(format!("{name}.x-jf-group must be 1–64 bytes of text"));
        }
        Some(g) => Some(g.to_owned()),
        None => None,
    };
    let kind = match object.get("type").and_then(Value::as_str) {
        Some("string") => PropertyKind::String,
        Some("integer") => PropertyKind::Integer,
        Some("number") => PropertyKind::Number,
        Some("boolean") => PropertyKind::Boolean,
        Some("array") if v12 => PropertyKind::Array,
        Some("array") => {
            return Err(format!(
                "property {name:?}: lists need api_version 1.2 (settings schema profile 1.2)"
            ));
        }
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
                .filter(|n| n.is_finite())
                .map(Some)
                .ok_or(format!("{name}.{key} must be a number")),
        }
    };
    let format = match object.get("format") {
        None => None,
        Some(_) if !is_string => return Err(format!("{name}.format only applies to strings")),
        Some(Value::String(f)) if f == "uri" => Some(Format::Uri),
        Some(Value::String(f)) if f == "email" => Some(Format::Email),
        Some(Value::String(f)) if f == "color" && v12 => Some(Format::Color),
        Some(Value::String(f)) if f == "color" => {
            return Err(format!(
                "{name}.format color needs api_version 1.2 (settings schema profile 1.2)"
            ));
        }
        Some(other) => return Err(format!("{name}.format {other} is not supported (uri, email)")),
    };
    let (choices, choice_titles) = match kind {
        PropertyKind::Array => {
            if object.get("uniqueItems") != Some(&Value::Bool(true)) {
                return Err(format!("{name}: a list needs \"uniqueItems\": true"));
            }
            let items = object
                .get("items")
                .and_then(Value::as_object)
                .ok_or(format!("{name}.items must be an object"))?;
            for key in items.keys() {
                if !matches!(key.as_str(), "type" | "enum" | "oneOf") {
                    return Err(format!("unsupported keyword {key:?} in {name}.items"));
                }
            }
            if items.get("type").and_then(Value::as_str) != Some("string") {
                return Err(format!("{name}.items must be strings"));
            }
            let (choices, titles) = choices_of(name, items.get("enum"), items.get("oneOf"))?;
            (
                Some(choices.ok_or(format!("{name}.items needs enum or oneOf"))?),
                titles,
            )
        }
        _ => {
            if object.contains_key("items") || object.contains_key("uniqueItems") {
                return Err(format!("{name}: items and uniqueItems only apply to lists"));
            }
            if !is_string && (object.contains_key("enum") || object.contains_key("oneOf")) {
                return Err(format!("{name}: enum and oneOf only apply to strings"));
            }
            choices_of(name, object.get("enum"), object.get("oneOf"))?
        }
    };
    let step = match object.get("multipleOf") {
        None => None,
        Some(_) if !is_number => return Err(format!("{name}.multipleOf only applies to numbers")),
        Some(v) => Some(
            v.as_f64()
                .filter(|n| n.is_finite() && *n > 0.0)
                .ok_or(format!("{name}.multipleOf must be a positive number"))?,
        ),
    };
    let property = Property {
        name: name.to_string(),
        kind,
        format,
        choices,
        choice_titles,
        default: object.get("default").cloned(),
        group,
        step,
        min_length: uint("minLength")?,
        max_length: uint("maxLength")?,
        minimum: bound("minimum")?,
        maximum: bound("maximum")?,
    };
    if property.step.is_some() && !property.is_slider() {
        return Err(format!("{name}.multipleOf needs both minimum and maximum"));
    }
    if let (Some(min), Some(max)) = (property.minimum, property.maximum)
        && min > max
    {
        return Err(format!("{name}.minimum is larger than its maximum"));
    }
    if let Some(default) = &property.default {
        property
            .check(default)
            .map_err(|e| format!("default of {name}: {e}"))?;
    }
    Ok(property)
}

/// The choices of `enum` or `oneOf` (`[{ "const": …, "title": … }]`), and
/// their titles.
fn choices_of(
    name: &str,
    enumeration: Option<&Value>,
    one_of: Option<&Value>,
) -> Result<(Option<Vec<String>>, Vec<Choice>), String> {
    match (enumeration, one_of) {
        (None, None) => Ok((None, Vec::new())),
        (Some(_), Some(_)) => Err(format!("{name}: use enum or oneOf, not both")),
        (Some(Value::Array(values)), None) if !values.is_empty() => {
            let choices = values
                .iter()
                .map(|v| v.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
                .ok_or(format!("{name}.enum must list strings"))?;
            check_distinct(name, &choices)?;
            Ok((Some(choices), Vec::new()))
        }
        (Some(_), None) => Err(format!("{name}.enum must be a non-empty array")),
        (None, Some(Value::Array(entries))) if !entries.is_empty() && entries.len() <= 64 => {
            let mut titles = Vec::new();
            for entry in entries {
                let object = entry
                    .as_object()
                    .ok_or(format!("{name}.oneOf must list objects"))?;
                if object.keys().any(|k| !matches!(k.as_str(), "const" | "title")) {
                    return Err(format!("{name}.oneOf entries hold only const and title"));
                }
                let value = object
                    .get("const")
                    .and_then(Value::as_str)
                    .ok_or(format!("{name}.oneOf entries need a string const"))?;
                let title = object
                    .get("title")
                    .and_then(Value::as_str)
                    .ok_or(format!("{name}.oneOf entries need a title"))?;
                titles.push(Choice {
                    value: value.to_owned(),
                    title: Some(title.to_owned()),
                });
            }
            let choices: Vec<String> = titles.iter().map(|c| c.value.clone()).collect();
            check_distinct(name, &choices)?;
            Ok((Some(choices), titles))
        }
        (None, Some(_)) => Err(format!("{name}.oneOf must list 1–64 choices")),
    }
}

fn check_distinct(name: &str, choices: &[String]) -> Result<(), String> {
    for (i, choice) in choices.iter().enumerate() {
        if choices[..i].contains(choice) {
            return Err(format!("{name} lists the choice {choice:?} twice"));
        }
    }
    Ok(())
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

    const SCHEMA_1_2: &str = r##"{
        "type": "object",
        "properties": {
            "corners": { "type": "number", "minimum": 0, "maximum": 2, "multipleOf": 0.25, "default": 1, "x-jf-group": "Look", "title": "Corners" },
            "size": { "type": "integer", "minimum": 0, "maximum": 10, "multipleOf": 2, "default": 4, "x-jf-group": "Look" },
            "highlight": { "type": "string", "format": "color", "default": "#FF8800", "x-jf-group": "Lyrics" },
            "layout": { "type": "string", "oneOf": [{ "const": "stage", "title": "Stage" }, { "const": "lyrics", "title": "Lyrics" }], "default": "stage" },
            "sidebar": { "type": "array", "items": { "type": "string", "enum": ["home", "albums", "songs"] }, "uniqueItems": true, "default": ["home", "albums"] },
            "home": { "type": "array", "items": { "type": "string", "oneOf": [{ "const": "summary", "title": "Summary" }, { "const": "recent", "title": "Recently Added" }] }, "uniqueItems": true }
        }
    }"##;

    #[test]
    fn profile_1_2_parses_validates_and_fills_defaults() {
        let schema = SettingsSchema::parse_with(SCHEMA_1_2, Profile::V1_2).unwrap();
        let corners = schema.property("corners").unwrap();
        assert_eq!(corners.step, Some(0.25));
        assert!(corners.is_slider());
        assert_eq!(corners.group.as_deref(), Some("Look"));
        assert_eq!(schema.property("highlight").unwrap().format, Some(Format::Color));
        let layout = schema.property("layout").unwrap();
        assert_eq!(
            layout.choices.as_deref(),
            Some(&["stage".to_string(), "lyrics".to_string()][..])
        );
        assert_eq!(layout.choice_titles[1].title.as_deref(), Some("Lyrics"));
        let sidebar = schema.property("sidebar").unwrap();
        assert_eq!(sidebar.kind, PropertyKind::Array);
        assert_eq!(
            schema.defaults(),
            json!({ "corners": 1, "size": 4, "highlight": "#FF8800", "layout": "stage", "sidebar": ["home", "albums"] })
        );
        let ok = schema
            .validate(&json!({ "corners": 1.75, "size": 10, "highlight": "#00aaFF", "layout": "lyrics", "sidebar": ["songs", "home"], "home": [] }))
            .unwrap();
        assert_eq!(
            ok["sidebar"],
            json!(["songs", "home"]),
            "the order is the person's"
        );
        for bad in [
            json!({ "corners": 1.3 }),
            json!({ "corners": 2.25 }),
            json!({ "size": 3 }),
            json!({ "highlight": "orange" }),
            json!({ "highlight": "#FF880" }),
            json!({ "layout": "Stage" }),
            json!({ "sidebar": ["home", "home"] }),
            json!({ "sidebar": ["search"] }),
            json!({ "sidebar": "home" }),
            json!({ "home": [1] }),
        ] {
            assert!(schema.validate(&bad).is_err(), "accepted {bad}");
        }
        // Stored values that no longer fit fall back to the defaults.
        let fitted = schema.fit(&json!({ "sidebar": ["home", "gone"], "corners": 0.5 }));
        assert_eq!(fitted["sidebar"], json!(["home", "albums"]));
        assert_eq!(fitted["corners"], json!(0.5));
    }

    #[test]
    fn profile_1_2_keywords_are_refused_in_older_packages() {
        let err = SettingsSchema::parse(SCHEMA_1_2).unwrap_err();
        assert!(err.contains("needs api_version 1.2"), "{err}");
        for property in [
            r#"{"type": "string", "format": "color"}"#,
            r#"{"type": "number", "minimum": 0, "maximum": 1, "multipleOf": 0.1}"#,
            r#"{"type": "array", "items": {"type": "string", "enum": ["a"]}, "uniqueItems": true}"#,
            r#"{"type": "string", "oneOf": [{"const": "a", "title": "A"}]}"#,
            r#"{"type": "string", "x-jf-group": "Look"}"#,
        ] {
            let text = format!(r#"{{"type": "object", "properties": {{"a": {property}}}}}"#);
            let err = SettingsSchema::parse(&text).unwrap_err();
            assert!(err.contains("api_version 1.2"), "{property}: {err}");
            assert!(
                SettingsSchema::parse_with(&text, Profile::V1_2).is_ok(),
                "{property}"
            );
        }
    }

    #[test]
    fn profile_1_2_rejects_bad_keywords() {
        for property in [
            r#"{"type": "number", "multipleOf": 0.1}"#,
            r#"{"type": "number", "minimum": 0, "maximum": 1, "multipleOf": 0}"#,
            r#"{"type": "number", "minimum": 2, "maximum": 1}"#,
            r#"{"type": "string", "multipleOf": 1}"#,
            r#"{"type": "array", "items": {"type": "string", "enum": ["a"]}}"#,
            r#"{"type": "array", "items": {"type": "integer", "enum": [1]}, "uniqueItems": true}"#,
            r#"{"type": "array", "items": {"type": "string"}, "uniqueItems": true}"#,
            r#"{"type": "string", "oneOf": [{"const": "a"}]}"#,
            r#"{"type": "string", "oneOf": [{"const": "a", "title": "A", "extra": 1}]}"#,
            r#"{"type": "string", "oneOf": [{"const": "a", "title": "A"}, {"const": "a", "title": "B"}]}"#,
            r#"{"type": "string", "enum": ["a"], "oneOf": [{"const": "a", "title": "A"}]}"#,
            r#"{"type": "string", "x-jf-group": ""}"#,
            r#"{"type": "string", "format": "color", "default": "red"}"#,
            r#"{"type": "boolean", "uniqueItems": true}"#,
        ] {
            let text = format!(r#"{{"type": "object", "properties": {{"a": {property}}}}}"#);
            assert!(
                SettingsSchema::parse_with(&text, Profile::V1_2).is_err(),
                "accepted {property}"
            );
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
