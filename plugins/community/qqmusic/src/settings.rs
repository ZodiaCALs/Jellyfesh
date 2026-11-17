//! The plugin's settings (`settings.schema.json`).

use jf_community_common::quality::{QualityChoice, flag};
use jf_plugin_sdk::Result;
use serde_json::Value;

/// What the person chose.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// `quality`: follow Jellyfesh or always use one quality.
    pub quality: QualityChoice,
    /// `translations`: merge QQ Music's lyric translations in.
    pub translations: bool,
    /// `mock_origin`, only in test builds (`mock-service` feature): the test
    /// suite's mock service, which every request goes to instead of QQ Music.
    /// The published package's settings schema has no such setting, so the
    /// host would refuse it, and its allowlist could not reach it anyway.
    #[cfg(feature = "mock-service")]
    pub mock_origin: Option<String>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            quality: QualityChoice::App,
            translations: true,
            #[cfg(feature = "mock-service")]
            mock_origin: None,
        }
    }
}

impl Settings {
    /// Reads the settings JSON.
    pub fn parse(value: &Value) -> Result<Settings> {
        Ok(Settings {
            quality: QualityChoice::from_settings(value)?,
            translations: flag(value, "translations", true),
            #[cfg(feature = "mock-service")]
            mock_origin: value
                .get("mock_origin")
                .and_then(Value::as_str)
                .map(|origin| origin.trim_end_matches('/').to_string())
                .filter(|origin| !origin.is_empty()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_plugin_sdk::Quality;
    use serde_json::json;

    #[test]
    fn defaults() {
        let settings = Settings::parse(&json!({})).unwrap();
        assert_eq!(settings, Settings::default());
        assert!(settings.translations);
        let settings = Settings::parse(&json!({ "quality": "high", "translations": false })).unwrap();
        assert_eq!(settings.quality, QualityChoice::Fixed(Quality::High));
        assert!(!settings.translations);
    }
}
