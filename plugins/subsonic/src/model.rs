//! The parts of the (Open)Subsonic JSON answers the plugin reads, after the
//! OpenSubsonic documentation (`opensubsonic.netlify.app`, sources at
//! github.com/opensubsonic/open-subsonic-api). Every field is optional or
//! defaulted so a server that leaves one out still works.

use serde::Deserialize;

use crate::lenient;

/// A song (`Child`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Song {
    #[serde(deserialize_with = "lenient::string")]
    pub id: String,
    #[serde(deserialize_with = "lenient::string")]
    pub title: String,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub album: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub artist: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub display_artist: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub album_id: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub artist_id: Option<String>,
    #[serde(deserialize_with = "lenient::opt_u32")]
    pub track: Option<u32>,
    #[serde(deserialize_with = "lenient::opt_u32")]
    pub disc_number: Option<u32>,
    #[serde(deserialize_with = "lenient::opt_u32")]
    pub year: Option<u32>,
    /// Seconds.
    #[serde(deserialize_with = "lenient::opt_u32")]
    pub duration: Option<u32>,
    /// kbit/s.
    #[serde(deserialize_with = "lenient::opt_u32")]
    pub bit_rate: Option<u32>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub suffix: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub content_type: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub cover_art: Option<String>,
    #[serde(deserialize_with = "lenient::strings")]
    pub isrc: Vec<String>,
    #[serde(deserialize_with = "lenient::opt_bool")]
    pub is_video: Option<bool>,
    #[serde(deserialize_with = "lenient::opt_bool")]
    pub is_dir: Option<bool>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub explicit_status: Option<String>,
}

/// An album (`AlbumID3`, with its songs for `getAlbum`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Album {
    #[serde(deserialize_with = "lenient::string")]
    pub id: String,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub name: Option<String>,
    /// Older servers name albums `title` or `album`.
    #[serde(deserialize_with = "lenient::opt_string")]
    pub title: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub album: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub artist: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub display_artist: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub artist_id: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub cover_art: Option<String>,
    #[serde(deserialize_with = "lenient::opt_u32")]
    pub song_count: Option<u32>,
    /// Seconds.
    #[serde(deserialize_with = "lenient::opt_u32")]
    pub duration: Option<u32>,
    #[serde(deserialize_with = "lenient::opt_u32")]
    pub year: Option<u32>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub explicit_status: Option<String>,
    #[serde(deserialize_with = "lenient::list")]
    pub song: Vec<Song>,
}

/// An artist (`ArtistID3`, with albums for `getArtist`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Artist {
    #[serde(deserialize_with = "lenient::string")]
    pub id: String,
    #[serde(deserialize_with = "lenient::string")]
    pub name: String,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub cover_art: Option<String>,
    #[serde(deserialize_with = "lenient::opt_u32")]
    pub album_count: Option<u32>,
    #[serde(deserialize_with = "lenient::list")]
    pub album: Vec<Album>,
}

/// A playlist (with its entries for `getPlaylist`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Playlist {
    #[serde(deserialize_with = "lenient::string")]
    pub id: String,
    #[serde(deserialize_with = "lenient::string")]
    pub name: String,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub owner: Option<String>,
    #[serde(deserialize_with = "lenient::opt_u32")]
    pub song_count: Option<u32>,
    /// Seconds.
    #[serde(deserialize_with = "lenient::opt_u32")]
    pub duration: Option<u32>,
    #[serde(deserialize_with = "lenient::opt_string")]
    pub cover_art: Option<String>,
    #[serde(deserialize_with = "lenient::list")]
    pub entry: Vec<Song>,
}

/// `getArtists`: artists grouped by index letter.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Indexes {
    #[serde(deserialize_with = "lenient::list")]
    pub index: Vec<Index>,
}

/// One index letter.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Index {
    #[serde(deserialize_with = "lenient::list")]
    pub artist: Vec<Artist>,
}

/// `search3` and `getStarred2`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Found {
    #[serde(deserialize_with = "lenient::list")]
    pub artist: Vec<Artist>,
    #[serde(deserialize_with = "lenient::list")]
    pub album: Vec<Album>,
    #[serde(deserialize_with = "lenient::list")]
    pub song: Vec<Song>,
}

/// `getAlbumList2`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AlbumList {
    #[serde(deserialize_with = "lenient::list")]
    pub album: Vec<Album>,
}

/// `getPlaylists`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Playlists {
    #[serde(deserialize_with = "lenient::list")]
    pub playlist: Vec<Playlist>,
}

/// `getSimilarSongs`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Songs {
    #[serde(deserialize_with = "lenient::list")]
    pub song: Vec<Song>,
}

/// `getLyricsBySongId`: `lyricsList`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LyricsList {
    #[serde(deserialize_with = "lenient::list")]
    pub structured_lyrics: Vec<StructuredLyrics>,
}

/// One set of lyrics (`structuredLyrics`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StructuredLyrics {
    #[serde(deserialize_with = "lenient::opt_string")]
    pub lang: Option<String>,
    #[serde(deserialize_with = "lenient::opt_bool")]
    pub synced: Option<bool>,
    /// Milliseconds; positive means the lyrics appear sooner.
    #[serde(deserialize_with = "lenient::opt_signed_f64")]
    pub offset: Option<f64>,
    /// `main`, `translation` or `pronunciation` (songLyrics v2); none means main.
    #[serde(deserialize_with = "lenient::opt_string")]
    pub kind: Option<String>,
    #[serde(deserialize_with = "lenient::list")]
    pub line: Vec<LyricLine>,
}

/// One line of structured lyrics.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LyricLine {
    /// Milliseconds from the start of the track; only for synced lyrics.
    #[serde(deserialize_with = "lenient::opt_f64")]
    pub start: Option<f64>,
    #[serde(deserialize_with = "lenient::string")]
    pub value: String,
}

/// `getLyrics`: plain text found by artist and title.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PlainLyrics {
    #[serde(deserialize_with = "lenient::string")]
    pub value: String,
}

/// `getUser`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct User {
    #[serde(deserialize_with = "lenient::string")]
    pub username: String,
    #[serde(deserialize_with = "lenient::opt_bool")]
    pub stream_role: Option<bool>,
}

/// `tokenInfo`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TokenInfo {
    #[serde(deserialize_with = "lenient::string")]
    pub username: String,
}

/// One entry of `openSubsonicExtensions`.
#[derive(Debug, Clone, Default, Deserialize, serde::Serialize, PartialEq)]
#[serde(default)]
pub struct Extension {
    #[serde(deserialize_with = "lenient::string")]
    pub name: String,
    pub versions: Vec<u32>,
}
