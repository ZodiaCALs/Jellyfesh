//! Streams through `/Audio/{id}/universal`.
//!
//! The plugin tells Jellyfin which formats the engine plays (`container`,
//! as `container|codec|codec` entries), the most bits per second the
//! quality allows (`maxStreamingBitrate`), and what to convert to otherwise
//! (`audioCodec`, `transcodingContainer`, progressive `http`: the engine
//! does not play HLS). The server then sends the original file, with range
//! support, or converts it on the fly. The plugin looks at the file first
//! (`fields=MediaSources`) and makes the same choice, so it can tell the
//! engine what will arrive:
//!
//! | Quality | Limit | Original file when |
//! | --- | --- | --- |
//! | lossless | none (100 Mbit/s) | its format is one the engine plays |
//! | high | 320 kbit/s | its format is playable and its bit rate within the limit |
//! | normal | 192 kbit/s | the same |
//! | low | 96 kbit/s | the same |
//!
//! Conversions run at the limit (320 kbit/s for lossless). The access token
//! travels in the `Authorization` header of every request, never in the URL.

use jf_plugin_sdk::prelude::*;

use crate::model::MediaSource;
use crate::settings::Codec;

/// Formats the engine plays, as Jellyfin's direct-play profiles: a container,
/// then the audio codecs allowed in it (any when none are listed).
pub const DIRECT_PLAY: &[&str] = &[
    "flac",
    "mp3",
    "aac",
    "m4a|aac|alac",
    "m4b|aac|alac",
    "mp4|aac|alac",
    "mov|aac|alac",
    "ogg|vorbis|opus|flac",
    "oga|vorbis|opus|flac",
    "opus",
    "webm|opus|vorbis",
    "webma|opus|vorbis",
    "mka|opus|vorbis|flac|aac|mp3",
    "wav",
    "aiff",
    "aif",
    "caf|alac|aac",
];

/// `maxStreamingBitrate` for lossless: high enough for any audio file.
const LOSSLESS_CAP: u32 = 100_000_000;
/// The conversion bit rate when nothing lower is asked for.
const TRANSCODE_MAX: u32 = 320_000;

/// The limit for a quality in bits per second (none: no limit).
pub fn limit(quality: Quality) -> Option<u32> {
    match quality {
        Quality::Low => Some(96_000),
        Quality::Normal => Some(192_000),
        Quality::High => Some(320_000),
        Quality::Lossless => None,
    }
}

/// Whether the engine plays `container` holding `codec`.
pub fn playable(container: &str, codec: Option<&str>) -> bool {
    let container = container.to_ascii_lowercase();
    let codec = codec.map(str::to_ascii_lowercase);
    // Jellyfin may report a list of names for one format ("mov,mp4,m4a").
    container.split(',').any(|name| {
        DIRECT_PLAY.iter().any(|entry| {
            let mut parts = entry.split('|');
            let entry_container = parts.next().unwrap_or_default();
            let codecs: Vec<&str> = parts.collect();
            entry_container == name.trim()
                && (codecs.is_empty() || codec.as_deref().is_some_and(|c| codecs.contains(&c)))
        })
    })
}

/// What will arrive, and what to ask for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The original file (otherwise a conversion).
    pub original: bool,
    /// `maxStreamingBitrate`.
    pub max_bitrate: u32,
    /// `audioBitRate` for a conversion.
    pub transcode_bitrate: u32,
    /// Codec hint for the engine, when known.
    pub codec: Option<String>,
    /// kbit/s, when known.
    pub bitrate_kbps: Option<u32>,
}

/// Decides how to stream a file.
pub fn plan(source: Option<&MediaSource>, quality: Quality, codec: Codec) -> Plan {
    let cap = limit(quality);
    let container = source.and_then(|s| s.container.clone()).unwrap_or_default();
    let audio_codec = source.and_then(|s| {
        s.media_streams
            .iter()
            .find(|m| m.kind.as_deref() == Some("Audio"))
            .and_then(|m| m.codec.clone())
    });
    let bitrate = source
        .and_then(|s| {
            s.bitrate.or_else(|| {
                s.media_streams
                    .iter()
                    .find(|m| m.kind.as_deref() == Some("Audio"))
                    .and_then(|m| m.bit_rate)
            })
        })
        .filter(|b| *b > 0)
        .and_then(|b| u32::try_from(b).ok());
    let is_playable = !container.is_empty() && playable(&container, audio_codec.as_deref());
    let transcode_bitrate = cap.unwrap_or(TRANSCODE_MAX).min(TRANSCODE_MAX);
    let original = Plan {
        original: true,
        max_bitrate: cap.unwrap_or(LOSSLESS_CAP),
        transcode_bitrate,
        codec: audio_codec
            .clone()
            .or_else(|| Some(container.clone()))
            .filter(|c| !c.is_empty())
            .map(|c| c.to_ascii_lowercase()),
        bitrate_kbps: bitrate.map(|b| b.div_ceil(1000)),
    };
    let convert = |known: bool| Plan {
        original: false,
        max_bitrate: cap.unwrap_or(LOSSLESS_CAP),
        transcode_bitrate,
        codec: known.then(|| codec.name().to_string()),
        bitrate_kbps: known.then_some(transcode_bitrate / 1000),
    };
    match (cap, bitrate) {
        (None, _) if is_playable => original,
        (None, _) => convert(true),
        (Some(cap), Some(rate)) if is_playable && rate <= cap => original,
        (Some(_), Some(_)) => convert(true),
        // An unknown bit rate: the server decides, so what arrives is unknown.
        (Some(_), None) if is_playable => convert(false),
        (Some(_), None) => convert(true),
    }
}

/// The query of a universal stream URL.
pub fn query(device_id: &str, plan: &Plan, codec: Codec) -> Vec<(&'static str, String)> {
    vec![
        ("deviceId", device_id.to_string()),
        ("container", DIRECT_PLAY.join(",")),
        ("audioCodec", codec.name().to_string()),
        ("transcodingContainer", codec.name().to_string()),
        ("transcodingProtocol", "http".to_string()),
        ("maxStreamingBitrate", plan.max_bitrate.to_string()),
        ("audioBitRate", plan.transcode_bitrate.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MediaStream;

    fn source(container: &str, codec: &str, bitrate: Option<i64>) -> MediaSource {
        MediaSource {
            container: Some(container.into()),
            bitrate,
            media_streams: vec![MediaStream {
                kind: Some("Audio".into()),
                codec: Some(codec.into()),
                bit_rate: bitrate,
            }],
        }
    }

    #[test]
    fn knows_what_the_engine_plays() {
        assert!(playable("flac", Some("flac")));
        assert!(playable("m4a", Some("alac")));
        assert!(!playable("m4a", Some("ac3")));
        assert!(playable("mov,mp4,m4a,3gp,3g2,mj2", Some("aac")));
        assert!(playable("ogg", Some("opus")));
        assert!(!playable("wma", Some("wmav2")));
        assert!(!playable("ape", None));
    }

    #[test]
    fn lossless_sends_the_original_when_playable() {
        let flac = plan(
            Some(&source("flac", "flac", Some(921_000))),
            Quality::Lossless,
            Codec::Aac,
        );
        assert!(flac.original);
        assert_eq!(
            (flac.codec.as_deref(), flac.bitrate_kbps),
            (Some("flac"), Some(921))
        );
        assert_eq!(flac.max_bitrate, LOSSLESS_CAP);
        let wma = plan(
            Some(&source("asf", "wmav2", Some(192_000))),
            Quality::Lossless,
            Codec::Mp3,
        );
        assert!(!wma.original);
        assert_eq!((wma.codec.as_deref(), wma.bitrate_kbps), (Some("mp3"), Some(320)));
        assert_eq!(wma.transcode_bitrate, 320_000);
    }

    #[test]
    fn limits_convert_only_above_them() {
        let mp3 = plan(
            Some(&source("mp3", "mp3", Some(128_000))),
            Quality::Normal,
            Codec::Aac,
        );
        assert!(mp3.original);
        let flac = plan(
            Some(&source("flac", "flac", Some(921_000))),
            Quality::Normal,
            Codec::Opus,
        );
        assert!(!flac.original);
        assert_eq!(
            (flac.codec.as_deref(), flac.bitrate_kbps),
            (Some("opus"), Some(192))
        );
        assert_eq!((flac.max_bitrate, flac.transcode_bitrate), (192_000, 192_000));
        let unknown = plan(Some(&source("mp3", "mp3", None)), Quality::Low, Codec::Aac);
        assert_eq!((unknown.codec, unknown.bitrate_kbps), (None, None));
        let nothing = plan(None, Quality::High, Codec::Aac);
        assert_eq!(nothing.codec.as_deref(), Some("aac"));
    }
}
