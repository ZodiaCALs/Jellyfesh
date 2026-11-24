//! The person's settings (see `settings.schema.json`).
//!
//! There is no quality setting: the app asks for a level with each stream
//! (ADR-0026), and a `quality` key left from an older version is ignored
//! (the host drops it when the plugin updates).

use jf_plugin_sdk::prelude::*;
use serde_json::Value;

/// What the server converts to when it has to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Codec {
    #[default]
    Aac,
    Mp3,
    Opus,
}

impl Codec {
    /// The `audioCodec` (and `transcodingContainer`) Jellyfin converts to,
    /// which is also the codec hint for the engine.
    pub fn name(self) -> &'static str {
        match self {
            Codec::Aac => "aac",
            Codec::Mp3 => "mp3",
            Codec::Opus => "opus",
        }
    }
}

/// Validated settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The server's base address without a trailing `/`, or none when it is
    /// not set yet.
    pub base: Option<String>,
    pub codec: Codec,
    /// `Device=` in the authorisation header.
    pub device_name: String,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            base: None,
            codec: Codec::Aac,
            device_name: "Jellyfesh".into(),
        }
    }
}

impl Settings {
    /// Reads the settings object the host delivers.
    pub fn parse(value: &Value) -> Result<Settings> {
        let base = match value.get("server_url").and_then(Value::as_str).map(str::trim) {
            None | Some("") => None,
            Some(url) => Some(normalise_base(url)?),
        };
        let codec = match value
            .get("transcode_codec")
            .and_then(Value::as_str)
            .unwrap_or("aac")
        {
            "aac" => Codec::Aac,
            "mp3" => Codec::Mp3,
            "opus" => Codec::Opus,
            other => return Err(PluginError::invalid_input(format!("unknown format {other:?}"))),
        };
        let device_name = value
            .get("device_name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or("Jellyfesh")
            .chars()
            .filter(|c| !c.is_control())
            .take(64)
            .collect();
        Ok(Settings {
            base,
            codec,
            device_name,
        })
    }
}

/// `https://Host:8096/jellyfin/web/#/home` → `https://Host:8096/jellyfin`:
/// people often paste the address of a page of the web app.
fn normalise_base(url: &str) -> Result<String> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(PluginError::invalid_input(
            "the server address must start with http:// or https://",
        ));
    }
    let without_query = url.split(['?', '#']).next().unwrap_or(url);
    let mut base = without_query.trim_end_matches('/').to_string();
    for page in ["/web/index.html", "/web"] {
        if base.to_ascii_lowercase().ends_with(page) {
            base.truncate(base.len() - page.len());
        }
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
            "server_url": " http://192.168.1.20:8096/jellyfin/web/#/home.html ",
            "transcode_codec": "opus",
            "device_name": "  Kitchen\n speaker "
        }))
        .unwrap();
        assert_eq!(s.base.as_deref(), Some("http://192.168.1.20:8096/jellyfin"));
        assert_eq!(s.codec.name(), "opus");
        assert_eq!(s.device_name, "Kitchen speaker");

        // A quality left from version 0.0.7 or older changes nothing.
        let s = Settings::parse(
            &json!({ "server_url": "https://jf.example.com/web/index.html", "quality": "low" }),
        )
        .unwrap();
        assert_eq!(s.base.as_deref(), Some("https://jf.example.com"));
        assert_eq!((s.codec, s.device_name.as_str()), (Codec::Aac, "Jellyfesh"));

        assert_eq!(Settings::parse(&json!({})).unwrap().base, None);
        assert!(Settings::parse(&json!({ "server_url": "jellyfin.local" })).is_err());
        assert!(Settings::parse(&json!({ "server_url": "http://x", "transcode_codec": "flac" })).is_err());
    }
}
