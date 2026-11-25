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
    /// The file's size in bytes.
    #[serde(deserialize_with = "lenient::opt_u64")]
    pub size: Option<u64>,
    /// The file's path on the server, relative to its music folder
    /// (`Artist/Album/01 Song.flac`); some servers leave it out.
    #[serde(deserialize_with = "lenient::opt_string")]
    pub path: Option<String>,
    /// When the person starred it (`2026-11-01T09:30:00Z`), if they did.
    #[serde(deserialize_with = "lenient::opt_string")]
    pub starred: Option<String>,
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
    #[serde(deserialize_with = "lenient::opt_string")]
    pub genre: Option<String>,
    /// OpenSubsonic: the composers as one text.
    #[serde(deserialize_with = "lenient::opt_string")]
    pub display_composer: Option<String>,
    /// OpenSubsonic: who did what (`composer`, `lyricist`, `arranger`, `producer`, …).
    #[serde(deserialize_with = "lenient::list")]
    pub contributors: Vec<Contributor>,
}

/// A name (`ItemGenre`, `RecordLabel`, a contributor's artist).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Named {
    #[serde(deserialize_with = "lenient::string")]
    pub name: String,
}

/// An OpenSubsonic contributor of a song.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Contributor {
    #[serde(deserialize_with = "lenient::string")]
    pub role: String,
    pub artist: Named,
}

/// An OpenSubsonic date (`ItemDate`): any of year, month and day.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ItemDate {
    #[serde(deserialize_with = "lenient::opt_u32")]
    pub year: Option<u32>,
    #[serde(deserialize_with = "lenient::opt_u32")]
    pub month: Option<u32>,
    #[serde(deserialize_with = "lenient::opt_u32")]
    pub day: Option<u32>,
}

impl ItemDate {
    /// `2021-03-05`, `2021-03` or `2021`.
    pub fn text(&self) -> Option<String> {
        let year = self.year.filter(|y| *y > 0)?;
        Some(
            match (
                self.month.filter(|m| (1..=12).contains(m)),
                self.day.filter(|d| (1..=31).contains(d)),
            ) {
                (Some(month), Some(day)) => format!("{year:04}-{month:02}-{day:02}"),
                (Some(month), None) => format!("{year:04}-{month:02}"),
                _ => format!("{year:04}"),
            },
        )
    }
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
    #[serde(deserialize_with = "lenient::opt_string")]
    pub genre: Option<String>,
    /// OpenSubsonic: every genre.
    #[serde(deserialize_with = "lenient::list")]
    pub genres: Vec<Named>,
    /// OpenSubsonic: the labels it was released on.
    #[serde(deserialize_with = "lenient::list")]
    pub record_labels: Vec<Named>,
    /// OpenSubsonic: when this release came out.
    #[serde(deserialize_with = "lenient::opt_object")]
    pub release_date: Option<ItemDate>,
    /// OpenSubsonic: the album's version ("Deluxe Edition").
    #[serde(deserialize_with = "lenient::opt_string")]
    pub version: Option<String>,
    /// When the person starred it, if they did.
    #[serde(deserialize_with = "lenient::opt_string")]
    pub starred: Option<String>,
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
    /// When the person starred them, if they did.
    #[serde(deserialize_with = "lenient::opt_string")]
    pub starred: Option<String>,
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
    /// The playlist's description.
    #[serde(deserialize_with = "lenient::opt_string")]
    pub comment: Option<String>,
}

/// `getArtistInfo2` (`artistInfo2`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArtistInfo {
    #[serde(deserialize_with = "lenient::opt_string")]
    pub biography: Option<String>,
}

/// `getAlbumInfo2` (`albumInfo`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AlbumInfo {
    #[serde(deserialize_with = "lenient::opt_string")]
    pub notes: Option<String>,
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
    /// Whether the account may download (Navidrome: whether downloads are
    /// on at all, its `EnableDownloads`).
    #[serde(deserialize_with = "lenient::opt_bool")]
    pub download_role: Option<bool>,
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
