//! The person's settings (see `settings.schema.json`).

use jf_plugin_sdk::prelude::*;
use serde_json::Value;

/// What to stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QualityChoice {
    /// The quality the app asks for (per network).
    #[default]
    App,
    /// Always this quality on this server.
    Fixed(Quality),
}

/// What the server converts to when it has to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Transcode {
    #[default]
    Mp3,
    Opus,
    Aac,
}

impl Transcode {
    /// The `format` parameter of `stream`, which is also the codec hint.
    pub fn format(self) -> &'static str {
        match self {
            Transcode::Mp3 => "mp3",
            Transcode::Opus => "opus",
            Transcode::Aac => "aac",
        }
    }
}

/// Validated settings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Settings {
    /// The server's base address without a trailing `/` (and without a
    /// trailing `/rest`), or none when it is not set yet.
    pub base: Option<String>,
    pub quality: QualityChoice,
    pub transcode: Transcode,
}

impl Settings {
    /// Reads the settings object the host delivers.
    pub fn parse(value: &Value) -> Result<Settings> {
        let base = match value.get("server_url").and_then(Value::as_str).map(str::trim) {
            None | Some("") => None,
            Some(url) => Some(normalise_base(url)?),
        };
        let quality = match value.get("quality").and_then(Value::as_str).unwrap_or("app") {
            "app" => QualityChoice::App,
            "lossless" => QualityChoice::Fixed(Quality::Lossless),
            "high" => QualityChoice::Fixed(Quality::High),
            "normal" => QualityChoice::Fixed(Quality::Normal),
            "low" => QualityChoice::Fixed(Quality::Low),
            other => return Err(PluginError::invalid_input(format!("unknown quality {other:?}"))),
        };
        let transcode = match value
            .get("transcode_format")
            .and_then(Value::as_str)
            .unwrap_or("mp3")
        {
            "mp3" => Transcode::Mp3,
            "opus" => Transcode::Opus,
            "aac" => Transcode::Aac,
            other => return Err(PluginError::invalid_input(format!("unknown format {other:?}"))),
        };
        Ok(Settings {
            base,
            quality,
            transcode,
        })
    }

    /// The quality to use when the app asks for `asked`.
    pub fn quality(&self, asked: Quality) -> Quality {
        match self.quality {
            QualityChoice::App => asked,
            QualityChoice::Fixed(quality) => quality,
        }
    }
}

/// `https://Host:4533/music/rest/` → `https://Host:4533/music`.
fn normalise_base(url: &str) -> Result<String> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(PluginError::invalid_input(
            "the server address must start with http:// or https://",
        ));
    }
    let without_query = url.split(['?', '#']).next().unwrap_or(url);
    let mut base = without_query.trim_end_matches('/').to_string();
    if base.to_ascii_lowercase().ends_with("/rest") {
        base.truncate(base.len() - "/rest".len());
    }
    Ok(base.trim_end_matches('/').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_and_normalises() {
        let s = Settings::parse(&json!({
            "server_url": " http://192.168.1.20:4533/navidrome/rest/ ",
            "quality": "high",
            "transcode_format": "opus"
        }))
        .unwrap();
        assert_eq!(s.base.as_deref(), Some("http://192.168.1.20:4533/navidrome"));
        assert_eq!(s.quality(Quality::Low), Quality::High);
        assert_eq!(s.transcode.format(), "opus");

        let s = Settings::parse(&json!({ "server_url": "https://music.example.com/" })).unwrap();
        assert_eq!(s.base.as_deref(), Some("https://music.example.com"));
        assert_eq!(
            s.quality(Quality::Low),
            Quality::Low,
            "follows the app by default"
        );
        assert_eq!(s.transcode, Transcode::Mp3);

        assert_eq!(Settings::parse(&json!({})).unwrap().base, None);
        assert!(Settings::parse(&json!({ "server_url": "ftp://x" })).is_err());
        assert!(Settings::parse(&json!({ "server_url": "http://x", "quality": "max" })).is_err());
    }
}
