//! Streaming quality (ADR-0026): the level streams are asked for, fitted to
//! the levels each source offers, and the plugins' own quality settings of
//! before v0.0.8, carried over once.

use std::path::Path;

use crate::types::StreamingQuality;

/// Plugins that had a Streaming Quality setting of their own before v0.0.8
/// (ADR-0026 §2): "app" followed the app, the other values were fixed.
const OWN_SETTING: [&str; 6] = [
    "org.jellyfesh.jellyfin",
    "org.jellyfesh.subsonic",
    "org.jellyfesh.netease",
    "org.jellyfesh.qqmusic",
    "community.unofficial.netease",
    "community.unofficial.qqmusic",
];

/// The level streams are asked for: the setting, with Auto as Lossless, or
/// as High on a connection the platform reports as metered (ADR-0026 §1).
pub(crate) fn streaming_level(setting: StreamingQuality, metered: bool) -> StreamingQuality {
    match setting {
        StreamingQuality::Auto if metered => StreamingQuality::High,
        StreamingQuality::Auto => StreamingQuality::Lossless,
        level => level,
    }
}

/// How good a level is (Lossless best).
fn rank(level: StreamingQuality) -> u8 {
    match level {
        StreamingQuality::Auto | StreamingQuality::Lossless => 4,
        StreamingQuality::High => 3,
        StreamingQuality::Normal => 2,
        StreamingQuality::Low => 1,
    }
}

/// The level to ask a source for: `level` when the source offers it (or
/// lists no levels), else the best level it offers below `level`, else the
/// lowest it offers. Auto is Lossless here.
pub(crate) fn fit(level: StreamingQuality, offered: &[StreamingQuality]) -> StreamingQuality {
    let level = match level {
        StreamingQuality::Auto => StreamingQuality::Lossless,
        level => level,
    };
    let offered: Vec<StreamingQuality> = offered
        .iter()
        .copied()
        .filter(|l| *l != StreamingQuality::Auto)
        .collect();
    if offered.is_empty() || offered.contains(&level) {
        return level;
    }
    offered
        .iter()
        .copied()
        .filter(|l| rank(*l) < rank(level))
        .max_by_key(|l| rank(*l))
        .or_else(|| offered.iter().copied().min_by_key(|l| rank(*l)))
        .unwrap_or(level)
}

/// The plugin API's level for a streaming level.
pub(crate) fn host_quality(level: StreamingQuality) -> jf_plugin_host::Quality {
    match level {
        StreamingQuality::Auto | StreamingQuality::Lossless => jf_plugin_host::Quality::Lossless,
        StreamingQuality::High => jf_plugin_host::Quality::High,
        StreamingQuality::Normal => jf_plugin_host::Quality::Normal,
        StreamingQuality::Low => jf_plugin_host::Quality::Low,
    }
}

/// The level the plugins' own Streaming Quality settings agree on
/// (ADR-0026 §2), read from their stored settings in `plugins_dir` (the
/// plugin host's folder) before their new versions drop the setting: the
/// fixed level every plugin that set one chose. `None` when none set one
/// (each followed the app) or they disagree: the app keeps Auto.
pub(crate) fn carried_over(plugins_dir: &Path) -> Option<StreamingQuality> {
    let mut agreed = None;
    for id in OWN_SETTING {
        let path = plugins_dir.join("data").join(id).join("settings.json");
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(settings) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        let level = match settings.get("quality").and_then(serde_json::Value::as_str) {
            Some("lossless") => StreamingQuality::Lossless,
            Some("high") => StreamingQuality::High,
            Some("normal") => StreamingQuality::Normal,
            Some("low") => StreamingQuality::Low,
            _ => continue,
        };
        match agreed {
            None => agreed = Some(level),
            Some(found) if found == level => {}
            Some(_) => return None,
        }
    }
    agreed
}

#[cfg(test)]
mod tests {
    use super::*;
    use StreamingQuality::{Auto, High, Lossless, Low, Normal};

    #[test]
    fn auto_is_lossless_except_on_a_metered_network() {
        assert_eq!(streaming_level(Auto, false), Lossless);
        assert_eq!(streaming_level(Auto, true), High);
        assert_eq!(streaming_level(Low, false), Low);
        assert_eq!(streaming_level(Lossless, true), Lossless, "a fixed level stays");
    }

    #[test]
    fn levels_fit_what_the_source_offers() {
        let offered = [Lossless, High, Low];
        assert_eq!(fit(High, &offered), High);
        assert_eq!(fit(Auto, &offered), Lossless);
        assert_eq!(fit(Normal, &offered), Low, "the best below");
        assert_eq!(fit(Lossless, &[High, Normal]), High);
        assert_eq!(
            fit(Low, &[Lossless, High]),
            High,
            "nothing below: the lowest offered"
        );
        assert_eq!(fit(Normal, &[]), Normal, "no levels listed: as asked");
        assert_eq!(fit(Auto, &[]), Lossless);
    }

    fn stored(dir: &Path, id: &str, json: &str) {
        let path = dir.join("data").join(id);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("settings.json"), json).unwrap();
    }

    #[test]
    fn the_plugins_old_settings_carry_over_when_they_agree() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(carried_over(dir.path()), None, "nothing stored");

        stored(dir.path(), "org.jellyfesh.netease", r#"{"quality": "app"}"#);
        stored(dir.path(), "org.jellyfesh.spotify", r#"{"quality": "low"}"#);
        assert_eq!(
            carried_over(dir.path()),
            None,
            "app follows the app; others never had one"
        );

        stored(
            dir.path(),
            "org.jellyfesh.jellyfin",
            r#"{"server_url": "http://192.168.1.20:8096", "quality": "high"}"#,
        );
        stored(dir.path(), "community.unofficial.qqmusic", "not json");
        assert_eq!(carried_over(dir.path()), Some(High));

        stored(dir.path(), "org.jellyfesh.subsonic", r#"{"quality": "high"}"#);
        assert_eq!(carried_over(dir.path()), Some(High), "they agree");

        stored(
            dir.path(),
            "community.unofficial.netease",
            r#"{"quality": "lossless"}"#,
        );
        assert_eq!(carried_over(dir.path()), None, "they disagree: Auto stays");
    }
}
