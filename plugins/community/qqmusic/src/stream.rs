//! Turning a track into a stream, only when QQ Music gives this account the
//! whole song.
//!
//! The song's details say which files it has (`file.size_*`) and its media
//! id; `vkey.GetVkeyServer` `CgiGetVkey` then answers, for each file asked
//! for by name (`M800<media id>.mp3`, …), a signed path (`purl`) when this
//! account may play that file, or nothing. The stream is one of the answer's
//! stream hosts (`sip`) plus that path.
//!
//! The plugin asks for the files of the chosen quality and below, best
//! first, and takes the first one the service grants, which is legitimate:
//! an account without a subscription often gets 128 kbit/s and not 320. It
//! refuses, with the reason, when nothing is granted, and never takes
//! anything but the file it asked for: the answer's `testfile2g` and
//! `testfilewifi` (previews) are never read, and a path for another file
//! than the one named is refused as a preview.

use jf_plugin_sdk::prelude::*;
use serde_json::{Value, json};

use crate::api;
use crate::catalog;
use crate::items::Id;
use crate::model::{Files, Track, text};

/// How long a granted stream address is used before asking again.
const STREAM_LIFETIME_MS: u64 = 20 * 60 * 1000;
/// Used when the answer names no stream host the plugin knows.
const DEFAULT_STREAM_BASE: &str = "https://ws.stream.qqmusic.qq.com/";

/// A file kind the stream service hands out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileKind {
    /// The file name's prefix.
    pub prefix: &'static str,
    /// Its extension.
    pub ext: &'static str,
    /// Codec hint.
    pub codec: &'static str,
    /// Nominal bit rate (none for lossless).
    pub kbps: Option<u32>,
}

pub const FLAC: FileKind = FileKind {
    prefix: "F000",
    ext: "flac",
    codec: "flac",
    kbps: None,
};
pub const MP3_320: FileKind = FileKind {
    prefix: "M800",
    ext: "mp3",
    codec: "mp3",
    kbps: Some(320),
};
pub const MP3_128: FileKind = FileKind {
    prefix: "M500",
    ext: "mp3",
    codec: "mp3",
    kbps: Some(128),
};
pub const AAC_96: FileKind = FileKind {
    prefix: "C400",
    ext: "m4a",
    codec: "aac",
    kbps: Some(96),
};

impl FileKind {
    /// The file's name for a media id.
    pub fn file_name(&self, media: &str) -> String {
        format!("{}{media}.{}", self.prefix, self.ext)
    }

    fn exists(&self, files: &Files) -> bool {
        match self.prefix {
            "F000" => files.size_flac > 0,
            "M800" => files.size_320mp3 > 0,
            "M500" => files.size_128mp3 > 0,
            _ => files.size_96aac > 0,
        }
    }
}

/// The files to ask for, best first, for a quality. When the song lists no
/// sizes at all, every kind of the quality is asked for and the service
/// decides.
pub fn candidates(quality: Quality, files: &Files) -> Vec<FileKind> {
    let order: &[FileKind] = match quality {
        Quality::Lossless => &[FLAC, MP3_320, MP3_128, AAC_96],
        Quality::High => &[MP3_320, MP3_128, AAC_96],
        Quality::Normal => &[MP3_128, AAC_96],
        Quality::Low => &[AAC_96, MP3_128],
    };
    if !files.any() {
        return order.to_vec();
    }
    order.iter().copied().filter(|kind| kind.exists(files)).collect()
}

/// What the service granted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grant {
    /// The path for this file kind.
    File(FileKind, String),
    /// Paths only for other files than the ones asked for: a preview.
    Preview,
    /// Nothing; the first non-zero result code, if any.
    Nothing(Option<i64>),
}

/// Reads the service's answer for the files asked for (`asked`, best first).
pub fn grant(data: &Value, asked: &[(FileKind, String)]) -> Grant {
    let infos = crate::model::list(data, "midurlinfo");
    let mut other_file = false;
    let mut result = None;
    for (index, (kind, name)) in asked.iter().enumerate() {
        let info = infos
            .iter()
            .find(|i| text(i, &["filename"]).as_deref() == Some(name.as_str()))
            .or_else(|| infos.get(index).filter(|i| text(i, &["filename"]).is_none()));
        let Some(info) = info else {
            continue;
        };
        if result.is_none() {
            result = info.get("result").and_then(Value::as_i64).filter(|r| *r != 0);
        }
        let Some(purl) = text(info, &["purl"]) else {
            continue;
        };
        let file = purl.split('?').next().unwrap_or_default();
        if file == name.as_str() {
            return Grant::File(*kind, purl);
        }
        other_file = true;
    }
    if other_file {
        Grant::Preview
    } else {
        Grant::Nothing(result)
    }
}

/// The message for a preview.
pub const PREVIEW: &str = "QQ Music offers your account only a preview of this track (it needs VIP or a \
     purchase). Jellyfesh plays whole tracks only.";

/// Why there is no stream, for the person and for later listings.
pub fn refusal(song: &Track, grant: &Grant) -> Unplayable {
    if matches!(grant, Grant::Preview) || song.pay.pay_play == 1 {
        return Unplayable {
            reason: UnplayableReason::Paywall,
            message: Some(if matches!(grant, Grant::Preview) {
                PREVIEW.into()
            } else {
                "This track needs a QQ Music subscription (VIP) or a purchase on your account.".into()
            }),
        };
    }
    let code = match grant {
        Grant::Nothing(Some(code)) => format!(" (code {code})"),
        _ => String::new(),
    };
    Unplayable {
        reason: UnplayableReason::Unavailable,
        message: Some(format!(
            "QQ Music gives your account no stream for this track{code}; it may be unavailable, or \
             limited to some regions."
        )),
    }
}

/// The first stream host in the answer the plugin trusts.
pub fn stream_base(data: &Value) -> String {
    crate::model::list(data, "sip")
        .iter()
        .filter_map(Value::as_str)
        .find_map(api::stream_base)
        .unwrap_or_else(|| DEFAULT_STREAM_BASE.to_string())
}

/// `playback.resolve-stream`.
pub fn resolve(item: &Item, quality: Quality) -> Result<StreamInfo> {
    let Some(Id::Track(mid)) = Id::parse(&item.id) else {
        return Err(PluginError::invalid_input("not a QQ Music track"));
    };
    let session = api::require_session()?;
    let quality = api::settings().quality.pick(quality);
    let song = catalog::song(mid)?;
    if !song.file.any() && song.file.media_mid.is_empty() {
        api::remember_refusal(
            mid,
            Unplayable {
                reason: UnplayableReason::Unavailable,
                message: Some(crate::items::NO_AUDIO.into()),
            },
        );
        return Err(PluginError::service(crate::items::NO_AUDIO));
    }
    let media = if song.file.media_mid.is_empty() {
        song.mid.clone()
    } else {
        song.file.media_mid.clone()
    };
    let asked: Vec<(FileKind, String)> = candidates(quality, &song.file)
        .into_iter()
        .map(|kind| (kind, kind.file_name(&media)))
        .collect();
    if asked.is_empty() {
        return Err(PluginError::service(crate::items::NO_AUDIO));
    }
    let count = asked.len();
    let data = api::call_signed_in(
        "vkey.GetVkeyServer",
        "CgiGetVkey",
        json!({
            "guid": api::guid()?,
            "songmid": vec![mid; count],
            "songtype": vec![0; count],
            "uin": session.musicid.to_string(),
            "loginflag": 1,
            "platform": "20",
            "filename": asked.iter().map(|(_, name)| name.as_str()).collect::<Vec<_>>(),
        }),
    )?;
    match grant(&data, &asked) {
        Grant::File(kind, purl) => {
            let mut stream = StreamInfo::new(api::physical(&format!("{}{purl}", stream_base(&data))));
            stream.codec = Some(kind.codec.to_string());
            stream.bitrate_kbps = kind.kbps;
            stream.expires_at_ms = Some(clock::now_utc_ms() + STREAM_LIFETIME_MS);
            // Streamed for this account only; never kept.
            stream.cacheable = false;
            Ok(stream)
        }
        refused => {
            let why = refusal(&song, &refused);
            let message = why.message.clone().unwrap_or_default();
            api::remember_refusal(mid, why);
            Err(PluginError::service(message))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn files(value: Value) -> Files {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn asks_for_existing_files_best_first() {
        let all = files(json!({ "size_96aac": 1, "size_128mp3": 1, "size_320mp3": 1, "size_flac": 1 }));
        assert_eq!(
            candidates(Quality::Lossless, &all),
            [FLAC, MP3_320, MP3_128, AAC_96]
        );
        assert_eq!(candidates(Quality::Low, &all), [AAC_96, MP3_128]);
        let no_flac = files(json!({ "size_128mp3": 1, "size_320mp3": 1 }));
        assert_eq!(candidates(Quality::Lossless, &no_flac), [MP3_320, MP3_128]);
        assert_eq!(candidates(Quality::Normal, &files(json!({}))), [MP3_128, AAC_96]);
        assert_eq!(MP3_320.file_name("0000Media"), "M8000000Media.mp3");
    }

    fn asked() -> Vec<(FileKind, String)> {
        vec![(MP3_320, "M800m.mp3".into()), (MP3_128, "M500m.mp3".into())]
    }

    #[test]
    fn takes_only_the_files_asked_for() {
        let granted = json!({ "midurlinfo": [
            { "filename": "M800m.mp3", "purl": "" },
            { "filename": "M500m.mp3", "purl": "M500m.mp3?vkey=v&guid=g" }
        ]});
        assert_eq!(
            grant(&granted, &asked()),
            Grant::File(MP3_128, "M500m.mp3?vkey=v&guid=g".into())
        );
        let other = json!({ "midurlinfo": [{ "filename": "M800m.mp3", "purl": "RS02m.mp3?vkey=v" }] });
        assert_eq!(grant(&other, &asked()), Grant::Preview);
        let none = json!({ "midurlinfo": [{ "filename": "M800m.mp3", "purl": "", "result": 104003 }],
                           "testfile2g": "RS02m.mp3?vkey=v" });
        assert_eq!(grant(&none, &asked()), Grant::Nothing(Some(104_003)));
        assert_eq!(grant(&json!({}), &asked()), Grant::Nothing(None));
    }

    #[test]
    fn explains_refusals() {
        let mut song = Track::default();
        assert_eq!(
            refusal(&song, &Grant::Nothing(None)).reason,
            UnplayableReason::Unavailable
        );
        assert!(
            refusal(&song, &Grant::Nothing(Some(5)))
                .message
                .unwrap()
                .contains("code 5")
        );
        assert_eq!(refusal(&song, &Grant::Preview).message.as_deref(), Some(PREVIEW));
        song.pay.pay_play = 1;
        assert_eq!(
            refusal(&song, &Grant::Nothing(None)).reason,
            UnplayableReason::Paywall
        );
    }

    #[test]
    fn picks_a_trusted_stream_host() {
        let data = json!({ "sip": ["http://aqqmusic.tc.qq.com/", "http://isure.stream.qqmusic.qq.com/"] });
        assert_eq!(stream_base(&data), "https://isure.stream.qqmusic.qq.com/");
        assert_eq!(stream_base(&json!({})), DEFAULT_STREAM_BASE);
    }
}
