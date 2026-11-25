//! The parts of Jellyfin's JSON answers the plugin reads, after the server's
//! API (Jellyfin 12.4: `Jellyfin.Api` controllers and `MediaBrowser.Model`
//! DTOs; the same shapes since 10.9). Property names are PascalCase.
//! Everything is optional or defaulted, and a `null` reads as the default,
//! so a field a server leaves out never fails a whole answer.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::de::{DeserializeOwned, Deserializer};

/// `null` (or a value of the wrong type) as the type's default.
fn lenient<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(d: D) -> Result<T, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

/// `BaseItemDto`: a track (`Audio`), album (`MusicAlbum`), artist
/// (`MusicArtist`), playlist, library view or folder.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct BaseItem {
    #[serde(deserialize_with = "lenient")]
    pub id: String,
    #[serde(deserialize_with = "lenient")]
    pub name: Option<String>,
    /// `Audio`, `MusicAlbum`, `MusicArtist`, `Playlist`, `CollectionFolder`, …
    #[serde(rename = "Type", deserialize_with = "lenient")]
    pub kind: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub album: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub album_id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub album_artist: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub album_artists: Vec<NameId>,
    #[serde(deserialize_with = "lenient")]
    pub artists: Vec<String>,
    #[serde(deserialize_with = "lenient")]
    pub artist_items: Vec<NameId>,
    #[serde(deserialize_with = "lenient")]
    pub album_primary_image_tag: Option<String>,
    /// Image type (`Primary`, `Backdrop`, …) → cache tag.
    #[serde(deserialize_with = "lenient")]
    pub image_tags: BTreeMap<String, String>,
    /// 100-nanosecond ticks.
    #[serde(deserialize_with = "lenient")]
    pub run_time_ticks: Option<i64>,
    /// Track number.
    #[serde(deserialize_with = "lenient")]
    pub index_number: Option<i64>,
    /// Disc number.
    #[serde(deserialize_with = "lenient")]
    pub parent_index_number: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    pub production_year: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    pub child_count: Option<i64>,
    /// `FileSystem`, `Remote`, `Virtual` (no file: missing from the library).
    #[serde(deserialize_with = "lenient")]
    pub location_type: Option<String>,
    /// `Full` or `None` (with `fields=PlayAccess`): whether the account may play it.
    #[serde(deserialize_with = "lenient")]
    pub play_access: Option<String>,
    /// With `fields=CanDownload`: whether the account may download it (its
    /// `EnableContentDownloading` policy, and a file to download).
    #[serde(deserialize_with = "lenient")]
    pub can_download: Option<bool>,
    #[serde(deserialize_with = "lenient")]
    pub media_type: Option<String>,
    /// For library views: `music`, `playlists`, `movies`, …
    #[serde(deserialize_with = "lenient")]
    pub collection_type: Option<String>,
    /// With `fields=MediaSources`.
    #[serde(deserialize_with = "lenient")]
    pub media_sources: Vec<MediaSource>,
    /// With `fields=Overview`: album notes, an artist's biography.
    #[serde(deserialize_with = "lenient")]
    pub overview: Option<String>,
    /// With `fields=Genres`.
    #[serde(deserialize_with = "lenient")]
    pub genres: Vec<String>,
    /// With `fields=People`: composers, lyricists and others.
    #[serde(deserialize_with = "lenient")]
    pub people: Vec<Person>,
    /// With `fields=Studios`: for music, the labels.
    #[serde(deserialize_with = "lenient")]
    pub studios: Vec<NameId>,
    /// When it was released (`2021-03-05T00:00:00.0000000Z`).
    #[serde(deserialize_with = "lenient")]
    pub premiere_date: Option<String>,
}

/// `BaseItemPerson`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Person {
    #[serde(deserialize_with = "lenient")]
    pub name: Option<String>,
    /// `PersonKind`: `Composer`, `Lyricist`, `Arranger`, `Producer`, …
    #[serde(rename = "Type", deserialize_with = "lenient")]
    pub kind: Option<String>,
}

/// `NameGuidPair`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct NameId {
    #[serde(deserialize_with = "lenient")]
    pub name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub id: String,
}

/// `MediaSourceInfo`: one version of a file.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MediaSource {
    #[serde(deserialize_with = "lenient")]
    pub container: Option<String>,
    /// The file's path on the server.
    #[serde(deserialize_with = "lenient")]
    pub path: Option<String>,
    /// The file's size in bytes.
    #[serde(deserialize_with = "lenient")]
    pub size: Option<i64>,
    /// Bits per second.
    #[serde(deserialize_with = "lenient")]
    pub bitrate: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    pub media_streams: Vec<MediaStream>,
}

/// `MediaStream`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MediaStream {
    /// `Audio`, `Video`, `EmbeddedImage`, …
    #[serde(rename = "Type", deserialize_with = "lenient")]
    pub kind: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub codec: Option<String>,
    /// Bits per second.
    #[serde(deserialize_with = "lenient")]
    pub bit_rate: Option<i64>,
}

/// `QueryResult<BaseItemDto>`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ItemList {
    #[serde(deserialize_with = "lenient")]
    pub items: Vec<BaseItem>,
    #[serde(deserialize_with = "lenient")]
    pub total_record_count: Option<i64>,
}

/// `AuthenticationResult`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct AuthResult {
    #[serde(deserialize_with = "lenient")]
    pub user: User,
    #[serde(deserialize_with = "lenient")]
    pub access_token: String,
    #[serde(deserialize_with = "lenient")]
    pub server_id: Option<String>,
}

/// `UserDto` (the parts read).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct User {
    #[serde(deserialize_with = "lenient")]
    pub name: String,
    #[serde(deserialize_with = "lenient")]
    pub id: String,
}

/// `PublicSystemInfo` (`/System/Info/Public`, no sign-in needed).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PublicInfo {
    #[serde(deserialize_with = "lenient")]
    pub server_name: Option<String>,
    /// `10.11.9`, `12.4.0`.
    #[serde(deserialize_with = "lenient")]
    pub version: Option<String>,
    /// `Jellyfin Server`.
    #[serde(deserialize_with = "lenient")]
    pub product_name: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub id: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub startup_wizard_completed: Option<bool>,
}

/// `QuickConnectResult`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct QuickConnect {
    #[serde(deserialize_with = "lenient")]
    pub authenticated: bool,
    #[serde(deserialize_with = "lenient")]
    pub secret: String,
    #[serde(deserialize_with = "lenient")]
    pub code: String,
}

/// `LyricDto` (`/Audio/{id}/Lyrics`, Jellyfin 10.9 and later).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Lyrics {
    #[serde(deserialize_with = "lenient")]
    pub metadata: LyricMetadata,
    #[serde(deserialize_with = "lenient")]
    pub lyrics: Vec<LyricLine>,
}

/// `LyricMetadata` (the parts read).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct LyricMetadata {
    /// "The lyric offset compared to audio in ticks".
    #[serde(deserialize_with = "lenient")]
    pub offset: Option<i64>,
    #[serde(deserialize_with = "lenient")]
    pub is_synced: Option<bool>,
}

/// `LyricLine`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct LyricLine {
    #[serde(deserialize_with = "lenient")]
    pub text: String,
    /// Start in ticks; none in unsynced lyrics.
    #[serde(deserialize_with = "lenient")]
    pub start: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_items_leniently() {
        let item: BaseItem = serde_json::from_str(
            r#"{"Id": "a1", "Name": "Song", "Type": "Audio", "Artists": null, "RunTimeTicks": 2450000000,
                "ImageTags": {"Primary": "t1"}, "IndexNumber": "x", "AlbumArtists": [{"Name": "A", "Id": "b2"}]}"#,
        )
        .unwrap();
        assert_eq!(item.id, "a1");
        assert_eq!(item.kind.as_deref(), Some("Audio"));
        assert!(item.artists.is_empty(), "null is an empty list");
        assert_eq!(item.index_number, None, "a wrong type is none");
        assert_eq!(item.run_time_ticks, Some(2_450_000_000));
        assert_eq!(item.image_tags["Primary"], "t1");
        assert_eq!(item.album_artists[0].id, "b2");
    }
}
