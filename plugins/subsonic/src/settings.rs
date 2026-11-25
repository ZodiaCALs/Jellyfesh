//! The person's settings (see `settings.schema.json`).
//!
//! There is no quality setting: the app asks for a level with each stream
//! (ADR-0026), and a `quality` key left from an older version is ignored
//! (the host drops it when the plugin updates).

use jf_plugin_sdk::prelude::*;
use serde_json::Value;

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
    pub transcode: Transcode,
}

impl Settings {
    /// Reads the settings object the host delivers.
    pub fn parse(value: &Value) -> Result<Settings> {
        let base = match value.get("server_url").and_then(Value::as_str).map(str::trim) {
            None | Some("") => None,
            Some(url) => Some(normalise_base(url)?),
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
        Ok(Settings { base, transcode })
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
            "transcode_format": "opus"
        }))
        .unwrap();
        assert_eq!(s.base.as_deref(), Some("http://192.168.1.20:4533/navidrome"));
        assert_eq!(s.transcode.format(), "opus");

        // A quality left from version 0.0.7 or older changes nothing.
        let s = Settings::parse(&json!({ "server_url": "https://music.example.com/", "quality": "low" }))
            .unwrap();
        assert_eq!(s.base.as_deref(), Some("https://music.example.com"));
        assert_eq!(s.transcode, Transcode::Mp3);

        assert_eq!(Settings::parse(&json!({})).unwrap().base, None);
        assert!(Settings::parse(&json!({ "server_url": "ftp://x" })).is_err());
        assert!(Settings::parse(&json!({ "server_url": "http://x", "transcode_format": "wma" })).is_err());
    }
}
