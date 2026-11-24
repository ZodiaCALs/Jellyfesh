//! Streaming quality (ADR-0026), as the core decides it:
//!
//! * the setting is [`StreamingQuality`]: Auto, Lossless, High, Normal or
//!   Low, `Auto` by default (`jf_core::StreamingQuality`, the same JSON);
//! * [`level`]: Auto asks for Lossless, or for High on a connection the
//!   platform reports as metered; the others ask for themselves. The page
//!   then steps down while the browser cannot play a format
//!   (`rules::next_quality`);
//! * [`menu`]: the quality menu's levels for what plays: Auto, then the
//!   levels the source's manifest lists (`content_policy.qualities`, best
//!   first); none for a file ("Files play as they are.") or a source that
//!   lists none ("Quality is set in {Service}.").

use jf_plugin_model::{ContentPolicy, Quality};
use serde::{Deserialize, Serialize};

/// Streaming Quality (Settings › Playback › Streaming and the bar's menu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamingQuality {
    /// Lossless, or High on a metered connection.
    #[default]
    Auto,
    Lossless,
    High,
    Normal,
    Low,
}

impl StreamingQuality {
    /// The level a source's own quality is in the menu.
    pub fn of(quality: Quality) -> StreamingQuality {
        match quality {
            Quality::Lossless => StreamingQuality::Lossless,
            Quality::High => StreamingQuality::High,
            Quality::Normal => StreamingQuality::Normal,
            Quality::Low => StreamingQuality::Low,
        }
    }
}

/// What a source is asked for with `setting` (`metered`: the connection is).
pub fn level(setting: StreamingQuality, metered: bool) -> Quality {
    match setting {
        StreamingQuality::Auto if metered => Quality::High,
        StreamingQuality::Auto | StreamingQuality::Lossless => Quality::Lossless,
        StreamingQuality::High => Quality::High,
        StreamingQuality::Normal => Quality::Normal,
        StreamingQuality::Low => Quality::Low,
    }
}

/// Why the quality menu offers no levels; the page words it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QualityNote {
    /// The library's songs and dropped files: "Files play as they are."
    FilesPlayAsTheyAre,
    /// A source that lists no levels: "Quality is set in {service}."
    SetInService { service: String },
}

/// The quality menu for what plays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Menu {
    /// Auto, then the source's levels best first; empty when there is no
    /// choice.
    pub qualities: Vec<StreamingQuality>,
    /// Why `qualities` is empty.
    pub note: Option<QualityNote>,
}

/// The menu for a source's stream (its name and content policy), or for a
/// file (`None`).
pub fn menu(source: Option<(&str, &ContentPolicy)>) -> Menu {
    let Some((name, policy)) = source else {
        return Menu {
            qualities: Vec::new(),
            note: Some(QualityNote::FilesPlayAsTheyAre),
        };
    };
    if policy.qualities.is_empty() {
        return Menu {
            qualities: Vec::new(),
            note: Some(QualityNote::SetInService {
                service: name.to_owned(),
            }),
        };
    }
    let mut levels = policy.qualities.clone();
    // Best first, once each (the manifest's order does not matter).
    levels.sort_by_key(|q| rank(*q));
    levels.dedup();
    let mut qualities = vec![StreamingQuality::Auto];
    qualities.extend(levels.into_iter().map(StreamingQuality::of));
    Menu {
        qualities,
        note: None,
    }
}

fn rank(quality: Quality) -> u8 {
    match quality {
        Quality::Lossless => 0,
        Quality::High => 1,
        Quality::Normal => 2,
        Quality::Low => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_is_lossless_unless_metered() {
        assert_eq!(level(StreamingQuality::Auto, false), Quality::Lossless);
        assert_eq!(level(StreamingQuality::Auto, true), Quality::High);
        assert_eq!(level(StreamingQuality::Low, true), Quality::Low);
        assert_eq!(level(StreamingQuality::Lossless, true), Quality::Lossless);
        assert_eq!(StreamingQuality::default(), StreamingQuality::Auto);
        assert_eq!(
            serde_json::to_string(&StreamingQuality::Normal).unwrap(),
            "\"normal\""
        );
    }

    #[test]
    fn the_menu_offers_auto_then_the_sources_levels() {
        let files = menu(None);
        assert!(files.qualities.is_empty());
        assert_eq!(files.note, Some(QualityNote::FilesPlayAsTheyAre));
        let none = ContentPolicy::default();
        assert_eq!(
            menu(Some(("Spotify", &none))).note,
            Some(QualityNote::SetInService {
                service: "Spotify".into()
            })
        );
        let policy = ContentPolicy {
            qualities: vec![Quality::Low, Quality::Lossless, Quality::High, Quality::Low],
            ..ContentPolicy::default()
        };
        let menu = menu(Some(("Jellyfin", &policy)));
        assert_eq!(
            menu.qualities,
            [
                StreamingQuality::Auto,
                StreamingQuality::Lossless,
                StreamingQuality::High,
                StreamingQuality::Low
            ]
        );
        assert_eq!(menu.note, None);
        assert_eq!(
            serde_json::to_value(QualityNote::SetInService { service: "X".into() }).unwrap(),
            serde_json::json!({"kind": "set_in_service", "service": "X"})
        );
    }
}
