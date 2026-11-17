//! The "Streaming Quality" setting: follow the quality chosen in Jellyfesh
//! for each network, or always use one.

use jf_plugin_sdk::{PluginError, Quality, Result};
use serde_json::Value;

/// What the person chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QualityChoice {
    /// The quality Jellyfesh asks for (by network).
    #[default]
    App,
    /// Always this one.
    Fixed(Quality),
}

impl QualityChoice {
    /// Reads the `quality` setting: `app`, `lossless`, `high`, `normal` or
    /// `low`; missing means `app`.
    pub fn from_settings(settings: &Value) -> Result<QualityChoice> {
        match settings.get("quality") {
            None | Some(Value::Null) => Ok(QualityChoice::App),
            Some(Value::String(text)) => match text.as_str() {
                "app" => Ok(QualityChoice::App),
                "lossless" => Ok(QualityChoice::Fixed(Quality::Lossless)),
                "high" => Ok(QualityChoice::Fixed(Quality::High)),
                "normal" => Ok(QualityChoice::Fixed(Quality::Normal)),
                "low" => Ok(QualityChoice::Fixed(Quality::Low)),
                other => Err(PluginError::invalid_input(format!("unknown quality {other:?}"))),
            },
            Some(other) => Err(PluginError::invalid_input(format!(
                "quality must be text, not {other}"
            ))),
        }
    }

    /// The quality to ask the service for when Jellyfesh asks for `requested`.
    pub fn pick(self, requested: Quality) -> Quality {
        match self {
            QualityChoice::App => requested,
            QualityChoice::Fixed(quality) => quality,
        }
    }
}

/// Reads a boolean setting, with a default when it is missing.
pub fn flag(settings: &Value, key: &str, default: bool) -> bool {
    settings.get(key).and_then(Value::as_bool).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_the_setting() {
        assert_eq!(
            QualityChoice::from_settings(&json!({})).unwrap(),
            QualityChoice::App
        );
        let fixed = QualityChoice::from_settings(&json!({ "quality": "low" })).unwrap();
        assert_eq!(fixed, QualityChoice::Fixed(Quality::Low));
        assert_eq!(fixed.pick(Quality::Lossless), Quality::Low);
        assert_eq!(QualityChoice::App.pick(Quality::High), Quality::High);
        assert!(QualityChoice::from_settings(&json!({ "quality": "ultra" })).is_err());
        assert!(QualityChoice::from_settings(&json!({ "quality": 3 })).is_err());
    }

    #[test]
    fn reads_flags() {
        assert!(flag(&json!({}), "translations", true));
        assert!(!flag(&json!({ "translations": false }), "translations", true));
    }
}
