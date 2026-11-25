//! Settings helpers. There is no "Streaming Quality" setting any more:
//! streams use the level Jellyfesh asks for (ADR-0026), from the levels the
//! manifest lists (`content_policy.qualities`), and a `quality` left from
//! an older version is ignored.

use serde_json::Value;

/// Reads a boolean setting, with a default when it is missing.
pub fn flag(settings: &Value, key: &str, default: bool) -> bool {
    settings.get(key).and_then(Value::as_bool).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_flags() {
        assert!(flag(&json!({}), "translations", true));
        assert!(!flag(&json!({ "translations": false }), "translations", true));
    }
}
