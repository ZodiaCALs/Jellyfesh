//! Streams and cover art.
//!
//! `stream` gets the song's format and bit rate first (`getSong`), so the
//! plugin can say what the engine will receive:
//!
//! | Quality | Asked of the server |
//! | --- | --- |
//! | lossless | `format=raw`: the original file, if the engine can play its format; otherwise converted at 320 kbit/s |
//! | high, normal, low | the original file when it is a format the engine plays at or below 320, 192 or 96 kbit/s; otherwise `format=<setting>&maxBitRate=<limit>` |
//!
//! Stream URLs carry the sign-in (Subsonic has no other way) and do not
//! expire.

use jf_plugin_sdk::prelude::*;

use crate::settings::Transcode;

/// File formats the engine decodes (by file suffix).
const PLAYABLE: &[&str] = &[
    "flac", "mp3", "m4a", "m4b", "mp4", "aac", "alac", "ogg", "oga", "opus", "wav", "wave", "aif", "aiff",
    "aifc", "caf", "mka", "webm",
];

/// The bit-rate limit for a quality, in kbit/s (none: no limit).
pub fn limit(quality: Quality) -> Option<u32> {
    match quality {
        Quality::Low => Some(96),
        Quality::Normal => Some(192),
        Quality::High => Some(320),
        Quality::Lossless => None,
    }
}

/// What to ask the server for, and what will arrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// `format` (`raw` for the original file).
    pub format: String,
    /// `maxBitRate`, when converting.
    pub max_bit_rate: Option<u32>,
    /// Codec hint for the engine, when known.
    pub codec: Option<String>,
    /// Bit rate in kbit/s, when known.
    pub bitrate: Option<u32>,
}

/// Decides how to stream a song with `suffix` and `bit_rate` (kbit/s).
pub fn plan(suffix: Option<&str>, bit_rate: Option<u32>, quality: Quality, transcode: Transcode) -> Plan {
    let suffix = suffix.map(str::to_ascii_lowercase);
    let playable = suffix.as_deref().is_some_and(|s| PLAYABLE.contains(&s));
    let original = || Plan {
        format: "raw".into(),
        max_bit_rate: None,
        codec: suffix.as_deref().and_then(codec_of_suffix).map(str::to_string),
        bitrate: bit_rate.filter(|b| *b > 0),
    };
    let convert = |cap: u32, known: bool| Plan {
        format: transcode.format().into(),
        max_bit_rate: Some(cap),
        // When the bit rate is unknown the server may still send the
        // original (it is already small enough), so the codec is unknown.
        codec: known.then(|| transcode.format().to_string()),
        bitrate: known.then_some(cap),
    };
    match limit(quality) {
        None if playable => original(),
        None => convert(320, true),
        Some(cap) => match bit_rate.filter(|b| *b > 0) {
            Some(rate) if playable && rate <= cap => original(),
            Some(_) => convert(cap, true),
            None if playable => convert(cap, false),
            None => convert(cap, true),
        },
    }
}

/// The codec (or, where the suffix does not say, the container) of a file
/// with this suffix. MP4 files can hold AAC or ALAC, so they get no hint.
fn codec_of_suffix(suffix: &str) -> Option<&'static str> {
    Some(match suffix {
        "flac" => "flac",
        "mp3" => "mp3",
        "aac" => "aac",
        "alac" => "alac",
        "opus" => "opus",
        "ogg" | "oga" => "ogg",
        "wav" | "wave" => "wav",
        "aif" | "aiff" | "aifc" => "aiff",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(suffix: &str, rate: Option<u32>, quality: Quality) -> Plan {
        plan(Some(suffix), rate, quality, Transcode::Mp3)
    }

    #[test]
    fn lossless_streams_the_original_when_the_engine_plays_it() {
        let flac = p("flac", Some(900), Quality::Lossless);
        assert_eq!(flac.format, "raw");
        assert_eq!((flac.codec.as_deref(), flac.bitrate), (Some("flac"), Some(900)));
        let wma = p("wma", Some(192), Quality::Lossless);
        assert_eq!(
            (wma.format.as_str(), wma.max_bit_rate, wma.codec.as_deref()),
            ("mp3", Some(320), Some("mp3"))
        );
    }

    #[test]
    fn lower_qualities_convert_only_above_the_limit() {
        let small = p("mp3", Some(128), Quality::Normal);
        assert_eq!((small.format.as_str(), small.max_bit_rate), ("raw", None));
        let big = p("flac", Some(900), Quality::Normal);
        assert_eq!(
            (big.format.as_str(), big.max_bit_rate, big.bitrate),
            ("mp3", Some(192), Some(192))
        );
        let low = plan(Some("mp3"), Some(320), Quality::Low, Transcode::Opus);
        assert_eq!((low.format.as_str(), low.max_bit_rate), ("opus", Some(96)));
        let unknown = p("mp3", None, Quality::High);
        assert_eq!(
            (unknown.format.as_str(), unknown.max_bit_rate),
            ("mp3", Some(320))
        );
        assert_eq!(unknown.codec, None, "the server may send the original");
    }
}
