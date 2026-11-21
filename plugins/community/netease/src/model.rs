//! The parts of NetEase Cloud Music's web answers the plugin reads.
//!
//! Field names are the service's. Every field is optional or has a default,
//! and numbers may come as JSON numbers or strings: an answer that lacks a
//! field, or changes a type, degrades an item instead of failing a whole page.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// A number given as a JSON number or a string; anything else is 0.
fn lenient_u64<'de, D: Deserializer<'de>>(de: D) -> Result<u64, D::Error> {
    Ok(match Value::deserialize(de)? {
        Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f as u64)).unwrap_or(0),
        Value::String(s) => s.trim().parse().unwrap_or(0),
        _ => 0,
    })
}

/// An optional number given as a JSON number or a string.
fn lenient_opt_i64<'de, D: Deserializer<'de>>(de: D) -> Result<Option<i64>, D::Error> {
    Ok(match Value::deserialize(de)? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    })
}

/// A list of strings; anything else in it (or no list at all) is dropped.
fn lenient_strings<'de, D: Deserializer<'de>>(de: D) -> Result<Vec<String>, D::Error> {
    Ok(match Value::deserialize(de)? {
        Value::Array(items) => items
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    })
}

/// A list of JSON values, or none for anything that is not a list.
fn lenient_list<'de, D: Deserializer<'de>>(de: D) -> Result<Vec<Value>, D::Error> {
    Ok(match Value::deserialize(de)? {
        Value::Array(items) => items,
        _ => Vec::new(),
    })
}

/// Reads an optional number as `u32`, dropping negative or huge values.
pub fn to_u32(value: Option<i64>) -> Option<u32> {
    value.and_then(|v| u32::try_from(v).ok())
}

/// An artist (`ar` of a song, `artists` of an album, search results).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Artist {
    #[serde(deserialize_with = "lenient_u64")]
    pub id: u64,
    pub name: Option<String>,
    #[serde(rename = "picUrl")]
    pub pic_url: Option<String>,
    #[serde(rename = "img1v1Url")]
    pub img1v1_url: Option<String>,
    #[serde(rename = "albumSize", deserialize_with = "lenient_opt_i64")]
    pub album_size: Option<i64>,
    /// A short biography (artist details).
    #[serde(rename = "briefDesc")]
    pub brief_desc: Option<String>,
    /// Other names.
    #[serde(deserialize_with = "lenient_strings")]
    pub alias: Vec<String>,
}

/// The album of a song (`al`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AlbumRef {
    #[serde(deserialize_with = "lenient_u64")]
    pub id: u64,
    pub name: Option<String>,
    #[serde(rename = "picUrl")]
    pub pic_url: Option<String>,
}

/// What the account may do with a song: `privilege` inside a song, or the
/// `privileges` list next to songs.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Privilege {
    #[serde(deserialize_with = "lenient_u64")]
    pub id: u64,
    /// 0 free, 1 VIP, 4 bought album, 8 free at standard quality.
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub fee: Option<i64>,
    /// Below 0: taken down or without rights.
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub st: Option<i64>,
    /// The best bit rate this account may play; 0: none at all.
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub pl: Option<i64>,
    /// The best bit rate there is.
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub maxbr: Option<i64>,
}

/// A song in the current answer format (`songs` of search, details,
/// playlists and albums).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Song {
    #[serde(deserialize_with = "lenient_u64")]
    pub id: u64,
    pub name: Option<String>,
    pub ar: Vec<Artist>,
    pub al: Option<AlbumRef>,
    /// Duration in ms.
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub dt: Option<i64>,
    /// Track number.
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub no: Option<i64>,
    /// Disc number, often as text ("01").
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub cd: Option<i64>,
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub fee: Option<i64>,
    /// Release date, ms since the epoch.
    #[serde(rename = "publishTime", deserialize_with = "lenient_opt_i64")]
    pub publish_time: Option<i64>,
    pub privilege: Option<Privilege>,
    /// Set when the service has no rights to the song.
    #[serde(rename = "noCopyrightRcmd")]
    pub no_copyright: Option<Value>,
}

/// An album (search results, album details, an artist's albums).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Album {
    #[serde(deserialize_with = "lenient_u64")]
    pub id: u64,
    pub name: Option<String>,
    #[serde(rename = "picUrl")]
    pub pic_url: Option<String>,
    pub artist: Option<Artist>,
    pub artists: Vec<Artist>,
    /// Number of songs.
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub size: Option<i64>,
    #[serde(rename = "publishTime", deserialize_with = "lenient_opt_i64")]
    pub publish_time: Option<i64>,
    /// The album's notes (album details).
    pub description: Option<String>,
    /// The record company.
    pub company: Option<String>,
    /// What kind of release: 录音室版 (studio), 现场版 (live), …
    #[serde(rename = "subType")]
    pub sub_type: Option<String>,
    pub info: Option<CommentInfo>,
}

/// An album's `info`: how many comments it has.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CommentInfo {
    #[serde(rename = "commentCount", deserialize_with = "lenient_opt_i64")]
    pub comment_count: Option<i64>,
}

/// Who made a playlist.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Creator {
    #[serde(rename = "userId", deserialize_with = "lenient_u64")]
    pub user_id: u64,
    pub nickname: Option<String>,
}

/// One entry of a playlist's `trackIds`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TrackId {
    #[serde(deserialize_with = "lenient_u64")]
    pub id: u64,
}

/// A playlist (search results, the account's playlists, details).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Playlist {
    #[serde(deserialize_with = "lenient_u64")]
    pub id: u64,
    pub name: Option<String>,
    #[serde(rename = "coverImgUrl")]
    pub cover_img_url: Option<String>,
    #[serde(rename = "trackCount", deserialize_with = "lenient_opt_i64")]
    pub track_count: Option<i64>,
    pub creator: Option<Creator>,
    /// 5: the account's liked songs.
    #[serde(rename = "specialType", deserialize_with = "lenient_opt_i64")]
    pub special_type: Option<i64>,
    #[serde(rename = "userId", deserialize_with = "lenient_u64")]
    pub user_id: u64,
    pub subscribed: Option<bool>,
    #[serde(rename = "trackIds")]
    pub track_ids: Vec<TrackId>,
    pub tracks: Vec<Song>,
    /// The cover where `coverImgUrl` is missing (recommended playlists).
    #[serde(rename = "picUrl")]
    pub pic_url: Option<String>,
    pub description: Option<String>,
    #[serde(deserialize_with = "lenient_strings")]
    pub tags: Vec<String>,
    #[serde(rename = "playCount", deserialize_with = "lenient_opt_i64")]
    pub play_count: Option<i64>,
    /// Ms since the epoch.
    #[serde(rename = "createTime", deserialize_with = "lenient_opt_i64")]
    pub create_time: Option<i64>,
    #[serde(rename = "commentCount", deserialize_with = "lenient_opt_i64")]
    pub comment_count: Option<i64>,
    /// How often a chart changes ("每天更新").
    #[serde(rename = "updateFrequency")]
    pub update_frequency: Option<String>,
}

impl Playlist {
    /// Its cover: `coverImgUrl`, else `picUrl`.
    pub fn cover(&self) -> Option<&str> {
        self.cover_img_url.as_deref().or(self.pic_url.as_deref())
    }
}

/// Who wrote a comment.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CommentUser {
    pub nickname: Option<String>,
    #[serde(rename = "avatarUrl")]
    pub avatar_url: Option<String>,
}

/// A comment's replies, where an answer counts them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Floor {
    #[serde(rename = "replyCount", deserialize_with = "lenient_opt_i64")]
    pub reply_count: Option<i64>,
}

/// One comment.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Comment {
    #[serde(rename = "commentId", deserialize_with = "lenient_u64")]
    pub comment_id: u64,
    pub user: Option<CommentUser>,
    pub content: Option<String>,
    /// Ms since the epoch.
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub time: Option<i64>,
    #[serde(rename = "likedCount", deserialize_with = "lenient_opt_i64")]
    pub liked_count: Option<i64>,
    #[serde(rename = "replyCount", deserialize_with = "lenient_opt_i64")]
    pub reply_count: Option<i64>,
    #[serde(rename = "showFloorComment")]
    pub floor: Option<Floor>,
}

/// A page of comments.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Comments {
    /// The highlighted comments (first page).
    #[serde(rename = "hotComments", deserialize_with = "lenient_list")]
    pub hot: Vec<Value>,
    #[serde(deserialize_with = "lenient_list")]
    pub comments: Vec<Value>,
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub total: Option<i64>,
    pub more: bool,
}

/// One entry of a stream URL answer.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UrlInfo {
    #[serde(deserialize_with = "lenient_u64")]
    pub id: u64,
    pub url: Option<String>,
    /// Bit rate in bit/s.
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub br: Option<i64>,
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub code: Option<i64>,
    /// Seconds until the URL stops working.
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub expi: Option<i64>,
    /// File type: `mp3`, `flac`, `m4a`, …
    #[serde(rename = "type")]
    pub kind: Option<String>,
    #[serde(rename = "encodeType")]
    pub encode_type: Option<String>,
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub fee: Option<i64>,
    /// Set when the URL is only a preview of the song.
    #[serde(rename = "freeTrialInfo")]
    pub free_trial_info: Option<Value>,
    /// Length of the file behind `url`, in ms.
    #[serde(deserialize_with = "lenient_opt_i64")]
    pub time: Option<i64>,
}

/// The text of one kind of lyrics.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LyricText {
    pub lyric: Option<String>,
}

/// The lyric answer.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Lyrics {
    pub lrc: Option<LyricText>,
    /// The translation, when there is one.
    pub tlyric: Option<LyricText>,
    /// Word-timed lyrics (NetEase's YRC), when there are some.
    pub yrc: Option<LyricText>,
    /// The translation timed for `yrc`.
    pub ytlrc: Option<LyricText>,
    /// No lyrics: an instrumental.
    pub nolyric: bool,
    /// Nobody has added lyrics yet.
    pub uncollected: bool,
}

/// The signed-in account.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Profile {
    #[serde(rename = "userId", deserialize_with = "lenient_u64")]
    pub user_id: u64,
    pub nickname: Option<String>,
}

/// The `account` of the account answer.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Account {
    #[serde(deserialize_with = "lenient_u64")]
    pub id: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_songs_leniently() {
        let song: Song = serde_json::from_value(json!({
            "id": "1001", "name": "Mock Song", "ar": [{ "id": 1, "name": "Mock Artist" }],
            "al": { "id": 2, "name": "Mock Album", "picUrl": "http://p1.music.126.net/x.jpg" },
            "dt": 201000, "no": 3, "cd": "01", "fee": 8, "publishTime": 1600000000000_i64,
            "privilege": { "id": 1001, "fee": 8, "st": 0, "pl": 128000, "maxbr": 999000 },
            "noCopyrightRcmd": null, "unknown": [1, 2]
        }))
        .unwrap();
        assert_eq!(song.id, 1001);
        assert_eq!(song.cd, Some(1));
        assert_eq!(song.dt, Some(201_000));
        assert_eq!(song.privilege.unwrap().pl, Some(128_000));
        assert!(song.no_copyright.is_none());
        let odd: Song = serde_json::from_value(json!({ "id": null, "cd": "A", "ar": [] })).unwrap();
        assert_eq!((odd.id, odd.cd), (0, None));
    }

    #[test]
    fn converts_numbers() {
        assert_eq!(to_u32(Some(5)), Some(5));
        assert_eq!(to_u32(Some(-1)), None);
        assert_eq!(to_u32(Some(i64::MAX)), None);
        assert_eq!(to_u32(None), None);
    }
}
