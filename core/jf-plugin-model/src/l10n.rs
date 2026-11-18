//! Package translations of the settings schema (API 1.2, ADR-0020).
//!
//! A package with `api_version = "1.2"` and a settings schema may carry
//! `l10n/<language>.json` entries (`l10n/zh-Hans.json`, `l10n/ja.json`): a
//! flat JSON object from these keys to the language's words:
//!
//! | Key | Translates |
//! | --- | --- |
//! | `title`, `description` | the schema's own |
//! | `properties.<name>.title` | a setting's title |
//! | `properties.<name>.description` | a setting's description |
//! | `properties.<name>.x-jf-group` | the heading of a setting's group |
//! | `choices.<value>` | the title of a `oneOf` choice with that `const` |
//!
//! Keys that name nothing in the schema are errors, so a typo never ships.
//! [`localize`] merges one translation into the schema text, keeping the
//! package's property order; what a translation leaves out stays English.

use std::collections::BTreeMap;

use serde::de::{Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};

use crate::schema::SettingsSchema;

/// Largest translation file.
pub const MAX_TRANSLATION: u64 = 64 * 1024;
/// Most entries in one translation.
const MAX_WORDS: usize = 512;
/// Longest translated text.
const MAX_TEXT: usize = 4096;
/// The folder translations live in, in a package.
pub const FOLDER: &str = "l10n/";

/// A language tag a package may translate into: a BCP 47 language with an
/// optional script and region (`ja`, `zh-Hans`, `pt-BR`, `zh-Hant-TW`).
pub fn valid_tag(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let Some(language) = parts.next() else {
        return false;
    };
    if !(2..=3).contains(&language.len()) || !language.bytes().all(|b| b.is_ascii_lowercase()) {
        return false;
    }
    let mut seen_script = false;
    let mut seen_region = false;
    for part in parts {
        let script = part.len() == 4
            && part.as_bytes()[0].is_ascii_uppercase()
            && part[1..].bytes().all(|b| b.is_ascii_lowercase());
        let region = (part.len() == 2 && part.bytes().all(|b| b.is_ascii_uppercase()))
            || (part.len() == 3 && part.bytes().all(|b| b.is_ascii_digit()));
        if script && !seen_script && !seen_region {
            seen_script = true;
        } else if region && !seen_region {
            seen_region = true;
        } else {
            return false;
        }
    }
    true
}

/// The language of a package entry `l10n/<tag>.json`, if `name` is one.
pub fn entry_language(name: &str) -> Option<&str> {
    let tag = name.strip_prefix(FOLDER)?.strip_suffix(".json")?;
    valid_tag(tag).then_some(tag)
}

/// The package entry name of a language's translation.
pub fn entry_name(tag: &str) -> String {
    format!("{FOLDER}{tag}.json")
}

/// One language's words for a settings schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    /// The language tag (`zh-Hans`).
    pub language: String,
    words: BTreeMap<String, String>,
}

impl Translation {
    /// Parses and checks `l10n/<language>.json` against the schema it
    /// translates.
    pub fn parse(language: &str, text: &str, schema: &SettingsSchema) -> Result<Translation, String> {
        if !valid_tag(language) {
            return Err(format!("{language:?} is not a language tag like ja or zh-Hans"));
        }
        let words: BTreeMap<String, String> =
            serde_json::from_str(text).map_err(|e| format!("not a JSON object of texts: {e}"))?;
        if words.len() > MAX_WORDS {
            return Err(format!("more than {MAX_WORDS} entries"));
        }
        for (key, value) in &words {
            if value.trim().is_empty()
                || value.len() > MAX_TEXT
                || value.chars().any(|c| c.is_control() && c != '\n')
            {
                return Err(format!(
                    "{key}: translations are 1–{MAX_TEXT} bytes of text without control characters"
                ));
            }
            if !names_something(key, schema) {
                return Err(format!("{key:?} names nothing in the settings schema"));
            }
        }
        Ok(Translation {
            language: language.to_owned(),
            words,
        })
    }

    /// The translation of `key`, if it has one.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.words.get(key).map(String::as_str)
    }

    /// How many texts it translates.
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// Whether it translates nothing.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
}

/// Whether a translation key refers to something the schema has.
fn names_something(key: &str, schema: &SettingsSchema) -> bool {
    if matches!(key, "title" | "description") {
        return true;
    }
    if let Some(value) = key.strip_prefix("choices.") {
        return schema
            .properties()
            .iter()
            .any(|p| p.choices.as_ref().is_some_and(|c| c.iter().any(|c| c == value)));
    }
    let Some(rest) = key.strip_prefix("properties.") else {
        return false;
    };
    let Some((name, field)) = rest.rsplit_once('.') else {
        return false;
    };
    schema.property(name).is_some() && matches!(field, "title" | "description" | "x-jf-group")
}

/// Of the languages a package translates into, the one to use for the app
/// language `wanted` (a BCP 47 tag such as `zh-Hans-CN`, `zh-TW` or `ja-JP`):
/// the same tag, the matching Chinese script, or a shorter tag. `None`
/// means English, the schema's own words.
pub fn best_language<'a>(available: &'a [String], wanted: &str) -> Option<&'a str> {
    let find = |tag: &str| {
        available
            .iter()
            .find(|a| a.eq_ignore_ascii_case(tag))
            .map(String::as_str)
    };
    if let Some(found) = find(wanted) {
        return Some(found);
    }
    let parts: Vec<&str> = wanted.split(['-', '_']).collect();
    if parts.first().is_some_and(|l| l.eq_ignore_ascii_case("zh")) {
        let has = |x: &str| parts.iter().skip(1).any(|p| p.eq_ignore_ascii_case(x));
        let traditional = has("Hant") || has("TW") || has("HK") || has("MO");
        if let Some(found) = find(if traditional { "zh-Hant" } else { "zh-Hans" }) {
            return Some(found);
        }
    }
    (1..parts.len()).rev().find_map(|n| find(&parts[..n].join("-")))
}

/// The schema text in the language of `translation` (the package's text
/// when there is none), with the properties in the package's order.
pub fn localize(schema: &SettingsSchema, translation: Option<&Translation>) -> String {
    let Some(translation) = translation.filter(|t| !t.is_empty()) else {
        return schema.text().to_owned();
    };
    let Ok(mut tree) = serde_json::from_str::<Ordered>(schema.text()) else {
        return schema.text().to_owned();
    };
    if let Ordered::Object(top) = &mut tree {
        for (key, value) in top.iter_mut() {
            match key.as_str() {
                "title" | "description" => replace(value, translation.get(key)),
                "properties" => {
                    if let Ordered::Object(properties) = value {
                        for (name, property) in properties.iter_mut() {
                            localize_property(name, property, translation);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    serde_json::to_string(&tree).unwrap_or_else(|_| schema.text().to_owned())
}

fn localize_property(name: &str, property: &mut Ordered, translation: &Translation) {
    let Ordered::Object(fields) = property else {
        return;
    };
    for (key, value) in fields.iter_mut() {
        match key.as_str() {
            "title" | "description" | "x-jf-group" => {
                replace(value, translation.get(&format!("properties.{name}.{key}")));
            }
            "oneOf" => localize_choices(value, translation),
            "items" => {
                if let Ordered::Object(items) = value {
                    for (key, value) in items.iter_mut() {
                        if key == "oneOf" {
                            localize_choices(value, translation);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn localize_choices(list: &mut Ordered, translation: &Translation) {
    let Ordered::Array(choices) = list else {
        return;
    };
    for choice in choices {
        let Ordered::Object(fields) = choice else {
            continue;
        };
        let value = fields.iter().find_map(|(k, v)| match (k.as_str(), v) {
            ("const", Ordered::String(s)) => Some(s.clone()),
            _ => None,
        });
        if let Some(value) = value {
            for (key, title) in fields.iter_mut() {
                if key == "title" {
                    replace(title, translation.get(&format!("choices.{value}")));
                }
            }
        }
    }
}

fn replace(value: &mut Ordered, text: Option<&str>) {
    if let Some(text) = text {
        *value = Ordered::String(text.to_owned());
    }
}

/// JSON that keeps the order of object keys (the workspace's `serde_json`
/// sorts them), so a localized schema lists the settings as the package does.
#[derive(Debug, Clone, PartialEq)]
enum Ordered {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Ordered>),
    Object(Vec<(String, Ordered)>),
}

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor;
        impl<'de> Visitor<'de> for OrderedVisitor {
            type Value = Ordered;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("JSON")
            }
            fn visit_unit<E>(self) -> Result<Ordered, E> {
                Ok(Ordered::Null)
            }
            fn visit_none<E>(self) -> Result<Ordered, E> {
                Ok(Ordered::Null)
            }
            fn visit_bool<E>(self, v: bool) -> Result<Ordered, E> {
                Ok(Ordered::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Ordered, E> {
                Ok(Ordered::Number(v.into()))
            }
            fn visit_u64<E>(self, v: u64) -> Result<Ordered, E> {
                Ok(Ordered::Number(v.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Ordered, E> {
                serde_json::Number::from_f64(v)
                    .map(Ordered::Number)
                    .ok_or_else(|| E::custom("not a finite number"))
            }
            fn visit_str<E>(self, v: &str) -> Result<Ordered, E> {
                Ok(Ordered::String(v.to_owned()))
            }
            fn visit_string<E>(self, v: String) -> Result<Ordered, E> {
                Ok(Ordered::String(v))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Ordered, A::Error> {
                let mut out = Vec::new();
                while let Some(value) = seq.next_element()? {
                    out.push(value);
                }
                Ok(Ordered::Array(out))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
                let mut out: Vec<(String, Ordered)> = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Ordered>()? {
                    out.retain(|(k, _)| *k != key);
                    out.push((key, value));
                }
                Ok(Ordered::Object(out))
            }
        }
        deserializer.deserialize_any(OrderedVisitor)
    }
}

impl Serialize for Ordered {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Ordered::Null => serializer.serialize_unit(),
            Ordered::Bool(b) => serializer.serialize_bool(*b),
            Ordered::Number(n) => n.serialize(serializer),
            Ordered::String(s) => serializer.serialize_str(s),
            Ordered::Array(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Ordered::Object(fields) => {
                let mut map = serializer.serialize_map(Some(fields.len()))?;
                for (key, value) in fields {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Profile;

    const SCHEMA: &str = r#"{
        "type": "object",
        "title": "Customize",
        "properties": {
            "corners": { "type": "number", "minimum": 0, "maximum": 2, "title": "Corners", "x-jf-group": "Look" },
            "layout": { "type": "string", "title": "Now Playing", "oneOf": [{ "const": "stage", "title": "Stage" }, { "const": "lyrics", "title": "Lyrics" }] },
            "home": { "type": "array", "uniqueItems": true, "title": "Home", "items": { "type": "string", "oneOf": [{ "const": "summary", "title": "Summary" }] } },
            "about": { "type": "boolean", "description": "Shows more" }
        }
    }"#;

    fn schema() -> SettingsSchema {
        SettingsSchema::parse_with(SCHEMA, Profile::V1_2).unwrap()
    }

    #[test]
    fn language_tags() {
        for tag in ["ja", "zh-Hans", "zh-Hant", "pt-BR", "zh-Hant-TW", "es-419"] {
            assert!(valid_tag(tag), "{tag}");
        }
        for tag in [
            "",
            "J",
            "JA",
            "zh-hans",
            "zh_Hans",
            "en-US-US",
            "../x",
            "zh-Hans-Hant",
            "english",
        ] {
            assert!(!valid_tag(tag), "{tag}");
        }
        assert_eq!(entry_language("l10n/zh-Hans.json"), Some("zh-Hans"));
        assert_eq!(entry_language("l10n/../x.json"), None);
        assert_eq!(entry_language("l10n/zh-Hans.txt"), None);
        assert_eq!(entry_name("ja"), "l10n/ja.json");
    }

    #[test]
    fn translations_are_checked_against_the_schema() {
        let schema = schema();
        let ok = r#"{"title": "自定义", "properties.corners.title": "圆角", "properties.corners.x-jf-group": "外观",
                     "properties.about.description": "显示更多", "choices.stage": "舞台", "choices.summary": "概览"}"#;
        assert_eq!(Translation::parse("zh-Hans", ok, &schema).unwrap().len(), 6);
        for bad in [
            r#"{"properties.missing.title": "x"}"#,
            r#"{"properties.corners.default": "x"}"#,
            r#"{"choices.floating": "x"}"#,
            r#"{"subtitle": "x"}"#,
            r#"{"title": ""}"#,
            r#"{"title": 3}"#,
            r#"["title"]"#,
        ] {
            assert!(Translation::parse("ja", bad, &schema).is_err(), "accepted {bad}");
        }
        assert!(Translation::parse("japanese", "{}", &schema).is_err());
    }

    #[test]
    fn localizing_keeps_the_order_and_falls_back_to_english() {
        let schema = schema();
        let words = r#"{"title": "カスタマイズ", "properties.corners.title": "角", "properties.corners.x-jf-group": "外観",
                        "choices.lyrics": "歌詞", "choices.summary": "概要"}"#;
        let translation = Translation::parse("ja", words, &schema).unwrap();
        let text = localize(&schema, Some(&translation));
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["title"], "カスタマイズ");
        assert_eq!(value["properties"]["corners"]["title"], "角");
        assert_eq!(value["properties"]["corners"]["x-jf-group"], "外観");
        assert_eq!(
            value["properties"]["layout"]["title"], "Now Playing",
            "untranslated stays English"
        );
        assert_eq!(value["properties"]["layout"]["oneOf"][0]["title"], "Stage");
        assert_eq!(value["properties"]["layout"]["oneOf"][1]["title"], "歌詞");
        assert_eq!(value["properties"]["home"]["items"]["oneOf"][0]["title"], "概要");
        let at = |name: &str| text.find(&format!("\"{name}\"")).unwrap();
        assert!(at("corners") < at("layout") && at("layout") < at("home") && at("home") < at("about"));
        // The localized schema is still a valid schema of the same profile.
        SettingsSchema::parse_with(&text, Profile::V1_2).unwrap();
        assert_eq!(localize(&schema, None), SCHEMA);
    }

    #[test]
    fn picks_the_best_language() {
        let available: Vec<String> = ["ja", "zh-Hans", "zh-Hant"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        for (wanted, found) in [
            ("ja", Some("ja")),
            ("ja-JP", Some("ja")),
            ("zh-Hans", Some("zh-Hans")),
            ("zh-Hans-CN", Some("zh-Hans")),
            ("zh-CN", Some("zh-Hans")),
            ("zh", Some("zh-Hans")),
            ("zh-TW", Some("zh-Hant")),
            ("zh-HK", Some("zh-Hant")),
            ("zh_Hant_TW", Some("zh-Hant")),
            ("en", None),
            ("en-US", None),
            ("fr", None),
        ] {
            assert_eq!(best_language(&available, wanted), found, "{wanted}");
        }
    }
}
