//! The parts of QQ Music's web answers the plugin reads.
//!
//! Songs come in one well-known shape (`Track`). Search results, playlists
//! and album lists are read field by field from JSON with [`text`] and
//! [`number`], which try each name the answers are known to use: QQ Music's
//! answers name the same thing differently from one endpoint to the next
//! (`albumMID`, `albummid`, `mid`), and a missing field must degrade an item,
//! not fail a page.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// A number given as a JSON number or a string; anything else is 0.
fn lenient_u64<'de, D: Deserializer<'de>>(de: D) -> Result<u64, D::Error> {
    Ok(to_u64(&Value::deserialize(de)?).unwrap_or(0))
}

/// Text given as a string or a number; anything else is empty.
fn lenient_text<'de, D: Deserializer<'de>>(de: D) -> Result<String, D::Error> {
    Ok(match Value::deserialize(de)? {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    })
}

fn to_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(n) => n
            .as_u64()
            .or_else(|| n.as_f64().filter(|f| *f >= 0.0).map(|f| f as u64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// The first of `keys` that holds non-empty text (or a number, as text).
pub fn text(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| match value.get(*key)? {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

/// The first of `keys` that holds a number (or a number as text).
pub fn number(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| to_u64(value.get(*key)?))
}

/// A singer of a song.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Singer {
    #[serde(deserialize_with = "lenient_u64")]
    pub id: u64,
    #[serde(deserialize_with = "lenient_text")]
    pub mid: String,
    #[serde(deserialize_with = "lenient_text")]
    pub name: String,
}

/// The album of a song.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AlbumRef {
    #[serde(deserialize_with = "lenient_u64")]
    pub id: u64,
    #[serde(deserialize_with = "lenient_text")]
    pub mid: String,
    #[serde(deserialize_with = "lenient_text")]
    pub name: String,
}

/// The files a song has (sizes in bytes; 0: no such file).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Files {
    #[serde(deserialize_with = "lenient_text")]
    pub media_mid: String,
    #[serde(deserialize_with = "lenient_u64")]
    pub size_96aac: u64,
    #[serde(deserialize_with = "lenient_u64")]
    pub size_128mp3: u64,
    #[serde(deserialize_with = "lenient_u64")]
    pub size_320mp3: u64,
    #[serde(deserialize_with = "lenient_u64")]
    pub size_flac: u64,
}

impl Files {
    /// Whether the song has any audio file the plugin knows.
    pub fn any(&self) -> bool {
        self.size_96aac + self.size_128mp3 + self.size_320mp3 + self.size_flac > 0
    }
}

/// What playing a song costs.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Pay {
    /// 1: playing needs a subscription or a purchase.
    #[serde(deserialize_with = "lenient_u64")]
    pub pay_play: u64,
    /// 1: included in the subscription.
    #[serde(deserialize_with = "lenient_u64")]
    pub pay_month: u64,
    #[serde(deserialize_with = "lenient_u64")]
    pub price_track: u64,
    #[serde(deserialize_with = "lenient_u64")]
    pub price_album: u64,
}

/// A song (search results, details, albums, playlists).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Track {
    #[serde(deserialize_with = "lenient_u64")]
    pub id: u64,
    #[serde(deserialize_with = "lenient_text")]
    pub mid: String,
    #[serde(deserialize_with = "lenient_text")]
    pub name: String,
    #[serde(deserialize_with = "lenient_text")]
    pub title: String,
    pub singer: Vec<Singer>,
    pub album: AlbumRef,
    /// Duration in seconds.
    #[serde(deserialize_with = "lenient_u64")]
    pub interval: u64,
    #[serde(deserialize_with = "lenient_text")]
    pub isrc: String,
    pub file: Files,
    pub pay: Pay,
    /// Position on the album.
    #[serde(deserialize_with = "lenient_u64")]
    pub index_album: u64,
    /// Release date, `2020-01-01`.
    #[serde(deserialize_with = "lenient_text")]
    pub time_public: String,
}

impl Track {
    /// The song's title, whichever field holds it.
    pub fn title(&self) -> &str {
        if self.name.trim().is_empty() {
            self.title.trim()
        } else {
            self.name.trim()
        }
    }
}

/// Reads one song, or `None` when the value is not one.
pub fn track(value: &Value) -> Option<Track> {
    let track: Track = serde_json::from_value(value.clone()).ok()?;
    (!track.mid.is_empty()).then_some(track)
}

/// Reads a list of songs, skipping entries that are not songs. Some answers
/// wrap each song (`{"songInfo": {…}}`).
pub fn tracks(value: &Value) -> Vec<Track> {
    value
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|entry| track(entry.get("songInfo").unwrap_or(entry)))
                .collect()
        })
        .unwrap_or_default()
}

/// The list at `value[key]`, or nothing.
pub fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_songs_leniently() {
        let song = track(&json!({
            "id": "4001", "mid": "0000MockSong01", "name": "Mock Song", "title": "Mock Song (title)",
            "singer": [{ "id": 1, "mid": "0000MockSinger", "name": "Mock Singer" }],
            "album": { "id": 9, "mid": "0000MockAlbum1", "name": "Mock Album" },
            "interval": "200", "file": { "media_mid": "0000MockMedia1", "size_128mp3": 3_200_000 },
            "pay": { "pay_play": 0 }, "index_album": 1, "time_public": "2020-05-01", "extra": [1]
        }))
        .unwrap();
        assert_eq!((song.id, song.interval), (4001, 200));
        assert_eq!(song.title(), "Mock Song");
        assert!(song.file.any());
        assert!(track(&json!({ "name": "no mid" })).is_none());
        let wrapped = tracks(&json!([{ "songInfo": { "mid": "a1" } }, { "mid": "b2" }, 5]));
        assert_eq!(
            wrapped.iter().map(|t| t.mid.as_str()).collect::<Vec<_>>(),
            ["a1", "b2"]
        );
    }

    #[test]
    fn reads_fields_by_any_name() {
        let entry = json!({ "albumMID": "", "albummid": "m1", "songNum": "12", "x": null });
        assert_eq!(
            text(&entry, &["albumMID", "albummid", "mid"]).as_deref(),
            Some("m1")
        );
        assert_eq!(number(&entry, &["song_count", "songNum"]), Some(12));
        assert_eq!(text(&entry, &["x"]), None);
        assert_eq!(number(&json!({ "n": -3 }), &["n"]), None);
        assert!(list(&entry, "missing").is_empty());
    }
}
