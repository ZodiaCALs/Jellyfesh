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
//!
//! Downloads (API 1.3, ADR-0025) always go through `download`, the call the
//! server's download role governs, never `stream`: the original file
//! (`format=raw`, which Navidrome needs to skip its own conversion), or, on
//! Navidrome, whose `download` also takes `format` and `bitrate`, a
//! conversion for High (320 kbit/s), Normal (192) and Low (128) when the
//! file is above that or in a format the engine does not play. Other
//! servers' `download` only sends originals, so they answer the original
//! for every quality.

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

/// The bit-rate limit of a download quality in kbit/s (none: the original).
pub fn download_limit(quality: DownloadQuality) -> Option<u32> {
    match quality {
        DownloadQuality::Original => None,
        DownloadQuality::High => Some(320),
        DownloadQuality::Normal => Some(192),
        DownloadQuality::Low => Some(128),
    }
}

/// What a download asks the server for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Download {
    /// The original file.
    Original,
    /// The server's conversion: `format` (also the file's extension) at
    /// `kbps`.
    Convert { format: &'static str, kbps: u32 },
}

/// Decides a download of a song with `suffix` and `bit_rate` (kbit/s).
/// `converts`: the server's `download` takes `format` and `bitrate`
/// (Navidrome). Conversions are Opus when the person's conversion format is
/// Opus, otherwise MP3, which every converting server offers and which can
/// carry tags (AAC arrives as a bare ADTS stream that cannot).
pub fn download(
    suffix: Option<&str>,
    bit_rate: Option<u32>,
    quality: DownloadQuality,
    converts: bool,
    transcode: Transcode,
) -> Download {
    let Some(cap) = download_limit(quality) else {
        return Download::Original;
    };
    let playable = suffix.is_some_and(|s| PLAYABLE.contains(&s.to_ascii_lowercase().as_str()));
    let small_enough = match bit_rate.filter(|b| *b > 0) {
        Some(rate) => rate <= cap,
        // The server knows the bit rate and might send the original anyway,
        // so a file of unknown bit rate comes as it is.
        None => true,
    };
    if !converts || (playable && small_enough) {
        return Download::Original;
    }
    let format = match transcode {
        Transcode::Opus => "opus",
        Transcode::Mp3 | Transcode::Aac => "mp3",
    };
    Download::Convert { format, kbps: cap }
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
    fn downloads_convert_only_on_navidrome_above_the_limit() {
        let d = |suffix, rate, quality, converts| download(Some(suffix), rate, quality, converts, Transcode::Mp3);
        assert_eq!(d("flac", Some(900), DownloadQuality::Original, true), Download::Original);
        assert_eq!(
            d("flac", Some(900), DownloadQuality::High, true),
            Download::Convert { format: "mp3", kbps: 320 }
        );
        assert_eq!(d("mp3", Some(320), DownloadQuality::High, true), Download::Original);
        assert_eq!(
            d("mp3", Some(320), DownloadQuality::Low, true),
            Download::Convert { format: "mp3", kbps: 128 }
        );
        assert_eq!(d("mp3", Some(128), DownloadQuality::Low, true), Download::Original);
        assert_eq!(d("mp3", None, DownloadQuality::Low, true), Download::Original);
        assert_eq!(
            d("wma", Some(192), DownloadQuality::Normal, true),
            Download::Convert { format: "mp3", kbps: 192 }
        );
        // Other servers' `download` sends only originals.
        assert_eq!(d("flac", Some(900), DownloadQuality::Low, false), Download::Original);
        assert_eq!(
            download(Some("flac"), Some(900), DownloadQuality::Normal, true, Transcode::Opus),
            Download::Convert { format: "opus", kbps: 192 }
        );
        assert_eq!(
            download(Some("flac"), Some(900), DownloadQuality::Normal, true, Transcode::Aac),
            Download::Convert { format: "mp3", kbps: 192 }
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
