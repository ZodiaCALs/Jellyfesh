//! Turning a track into a stream, only when NetEase gives this account the
//! whole song.
//!
//! `weapi/song/enhance/player/url/v1` (`ids`, `level`) answers with a URL,
//! its bit rate, type and lifetime, or no URL. If it fails, the older
//! `weapi/song/enhance/player/url` (`ids`, `br`) is asked instead. Whether
//! these web endpoints still give stream URLs could not be checked from here
//! (see the README).
//!
//! The plugin refuses, with the reason, whenever the answer is not the full
//! song for this account:
//!
//! * no URL: a subscription or purchase is needed (`fee` 1 or 4), or the song
//!   is unavailable;
//! * a URL marked as a preview (`freeTrialInfo`), or a file much shorter than
//!   the song: NetEase offers non-subscribers a clip of VIP songs, and
//!   Jellyfesh never plays a clip in place of the song.

use jf_plugin_sdk::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api;
use crate::catalog;
use crate::items::Id;
use crate::model::UrlInfo;

/// Keep a margin before the URL's own expiry.
const EXPIRY_MARGIN_MS: u64 = 60_000;
/// A file this much shorter than the song is a preview.
const PREVIEW_TOLERANCE_MS: u64 = 10_000;

/// The service's quality level and the matching bit rate for a quality.
pub fn level(quality: Quality) -> (&'static str, u32) {
    match quality {
        Quality::Low => ("standard", 128_000),
        Quality::Normal => ("higher", 192_000),
        Quality::High => ("exhigh", 320_000),
        Quality::Lossless => ("lossless", 999_000),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UrlAnswer {
    data: Vec<Value>,
}

/// Asks for the stream of song `id`: the current endpoint first, the older
/// one if that fails.
fn url_info(id: u64, quality: Quality) -> Result<UrlInfo> {
    let (level, bit_rate) = level(quality);
    let ids = format!("[{id}]");
    let first = api::call_as::<UrlAnswer>(
        "/song/enhance/player/url/v1",
        json!({ "ids": ids, "level": level, "encodeType": "flac" }),
    );
    let answer = match first {
        Ok(answer) if !answer.data.is_empty() => answer,
        Err(error @ (PluginError::AuthRequired | PluginError::RateLimited | PluginError::Network(_))) => {
            return Err(error);
        }
        _ => api::call_as::<UrlAnswer>("/song/enhance/player/url", json!({ "ids": ids, "br": bit_rate }))?,
    };
    catalog::list_of::<UrlInfo>(Value::Array(answer.data))
        .into_iter()
        .find(|info| info.id == id || info.id == 0)
        .ok_or(PluginError::NotFound)
}

/// Why there is no stream, in words, from what the answer says.
pub fn refusal(info: &UrlInfo) -> String {
    match info.fee {
        Some(1) => "This track needs a NetEase Cloud Music VIP subscription on your account.".into(),
        Some(4) => "This track needs its album bought on NetEase Cloud Music.".into(),
        _ => match info.code {
            Some(404) => "NetEase Cloud Music has no stream for this track.".into(),
            Some(code) if code != 200 => format!(
                "NetEase Cloud Music does not let your account play this track (code {code}); it may be \
                 unavailable, or limited to some regions."
            ),
            _ => "NetEase Cloud Music does not let your account play this track; it may be unavailable, or \
                  limited to some regions."
                .into(),
        },
    }
}

/// The message for a URL that is only a preview.
pub const PREVIEW: &str = "NetEase Cloud Music offers your account only a preview of this track (it needs VIP or \
     a purchase). Jellyfesh plays whole tracks only.";

/// Whether the file is a preview: marked as one, or much shorter than the
/// song (`song_ms`, when known).
pub fn is_preview(info: &UrlInfo, song_ms: Option<u64>) -> bool {
    if info.free_trial_info.as_ref().is_some_and(|v| !v.is_null()) {
        return true;
    }
    match (info.time.and_then(|t| u64::try_from(t).ok()), song_ms) {
        (Some(file), Some(song)) if file > 0 && song > 0 => {
            let tolerance = PREVIEW_TOLERANCE_MS.max(song / 10);
            file + tolerance < song
        }
        _ => false,
    }
}

/// A codec hint from the file type.
fn codec(info: &UrlInfo) -> Option<String> {
    let kind = info
        .kind
        .as_deref()
        .or(info.encode_type.as_deref())?
        .to_ascii_lowercase();
    Some(
        match kind.as_str() {
            "mp3" => "mp3",
            "flac" => "flac",
            "m4a" | "aac" | "mp4" => "aac",
            "ogg" => "vorbis",
            _ => return None,
        }
        .to_string(),
    )
}

/// `playback.resolve-stream`.
pub fn resolve(item: &Item, quality: Quality) -> Result<StreamInfo> {
    let Some(Id::Track(id)) = Id::parse(&item.id) else {
        return Err(PluginError::invalid_input("not a NetEase Cloud Music track"));
    };
    api::require_session()?;
    // The level Jellyfesh asks for (ADR-0026): no quality setting any more.
    let info = url_info(id, quality)?;
    let Some(url) = info.url.as_deref().map(str::trim).filter(|u| !u.is_empty()) else {
        return Err(PluginError::service(refusal(&info)));
    };
    let mut song_ms = item.duration_ms.map(u64::from);
    if song_ms.is_none() && info.time.is_some() {
        song_ms = catalog::song(id)?.dt.and_then(|d| u64::try_from(d).ok());
    }
    if is_preview(&info, song_ms) {
        return Err(PluginError::service(PREVIEW));
    }
    let Some(https) = api::media_url(url) else {
        jf_plugin_sdk::warn!("stream URL on an unexpected host; refusing it");
        return Err(PluginError::service(
            "NetEase Cloud Music gave a stream address this plugin does not trust.",
        ));
    };
    let mut stream = StreamInfo::new(api::physical(&https));
    stream.codec = codec(&info);
    stream.bitrate_kbps = info
        .br
        .and_then(|br| u32::try_from(br / 1000).ok())
        .filter(|kbps| *kbps > 0);
    stream.expires_at_ms = info
        .expi
        .and_then(|s| u64::try_from(s).ok())
        .filter(|s| *s > 0)
        .map(|s| clock::now_utc_ms() + (s * 1000).saturating_sub(EXPIRY_MARGIN_MS));
    // Streamed for this account only; never kept.
    stream.cacheable = false;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(value: Value) -> UrlInfo {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn maps_qualities() {
        assert_eq!(level(Quality::Low), ("standard", 128_000));
        assert_eq!(level(Quality::Lossless), ("lossless", 999_000));
    }

    #[test]
    fn spots_previews() {
        let full = info(json!({ "id": 1, "url": "x", "time": 200_000 }));
        assert!(!is_preview(&full, Some(201_000)));
        assert!(!is_preview(&full, None));
        let marked = info(json!({ "id": 1, "url": "x", "freeTrialInfo": { "start": 0, "end": 30 } }));
        assert!(is_preview(&marked, None));
        let short = info(json!({ "id": 1, "url": "x", "time": 30_000 }));
        assert!(is_preview(&short, Some(200_000)));
        let null_trial = info(json!({ "id": 1, "url": "x", "freeTrialInfo": null, "time": 30_000 }));
        assert!(!is_preview(&null_trial, Some(31_000)));
    }

    #[test]
    fn explains_refusals() {
        assert!(refusal(&info(json!({ "fee": 1 }))).contains("VIP"));
        assert!(refusal(&info(json!({ "fee": 4 }))).contains("bought"));
        assert!(refusal(&info(json!({ "code": 404 }))).contains("no stream"));
        assert!(refusal(&info(json!({ "code": -110 }))).contains("-110"));
    }

    #[test]
    fn codecs() {
        assert_eq!(codec(&info(json!({ "type": "MP3" }))).as_deref(), Some("mp3"));
        assert_eq!(
            codec(&info(json!({ "encodeType": "flac" }))).as_deref(),
            Some("flac")
        );
        assert_eq!(codec(&info(json!({ "type": "m4a" }))).as_deref(), Some("aac"));
        assert_eq!(codec(&info(json!({ "type": "wav" }))), None);
    }
}
