//! Jellyfesh Web's music library: the catalogue of the folders the person added. The page reads
//! the files through the browser and keeps the tracks in IndexedDB; this groups, sorts and
//! searches them by the native library's rules (`jf-library`, which needs SQLite and the file
//! system, so the rules are mirrored here and `tests/mirrors.rs` compares them with the
//! originals):
//!
//! * text is matched after NFKC, lowercasing and removing Latin, Greek and Cyrillic
//!   diacritics, with whitespace collapsed ([`normalize`]);
//! * an album is its title with its album artist, else its first track artist, so "A feat. B"
//!   stays on A's album;
//! * a track's artists are its ARTISTS list, else its several ARTIST values, else its one ARTIST
//!   value split at `;`, ` / ` and featuring markers ([`split_artists`]), duplicates dropped;
//! * the sort orders of `TrackSort` and `AlbumSort`: unknown artists, albums and years last in
//!   both directions;
//! * search in tiers: exact title, title prefix, then every piece found in the title, artist or
//!   album (the native library ranks the last tier with full-text indexes; here a match in the
//!   title ranks above one in the artist, then the album).

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::rules::ReplayGainInfo;

/// Normalises text for matching (`jf_library` `normalize`).
pub fn normalize(text: &str) -> String {
    let lower = text.nfkc().collect::<String>().to_lowercase();
    let folded: String = lower
        .nfd()
        .filter(|c| !('\u{0300}'..='\u{036F}').contains(c))
        .nfc()
        .collect();
    let mut out = String::with_capacity(folded.len());
    for word in folded.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// Splits an artist tag into artists, conservatively (`jf_library` `split_artists`): at `;`,
/// `；`, ` / ` and featuring markers, never at `&`, `,`, `and`, `x` or `vs.`.
pub fn split_artists(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let (main, featured) = split_featuring(text);
    for piece in [Some(main), featured].into_iter().flatten() {
        for name in piece.split([';', '；']).flat_map(|chunk| chunk.split(" / ")) {
            let name = name.trim();
            if !name.is_empty() {
                parts.push(name.to_owned());
            }
        }
    }
    parts
}

fn split_featuring(text: &str) -> (&str, Option<&str>) {
    const MARKERS: &[&str] = &["feat.", "feat", "ft.", "featuring"];
    let lower = text.to_ascii_lowercase();
    let mut best: Option<(usize, usize, Option<char>)> = None;
    for marker in MARKERS {
        for (start, _) in lower.match_indices(marker) {
            let end = start + marker.len();
            let before = lower[..start].chars().next_back();
            let after = lower[end..].chars().next();
            let opener = match before {
                Some(c @ ('(' | '[')) => Some(c),
                Some(c) if c.is_whitespace() => None,
                _ => continue,
            };
            if !after.is_some_and(char::is_whitespace) {
                continue;
            }
            if best.is_none_or(|(best_start, ..)| start < best_start) {
                best = Some((start, end, opener));
            }
        }
    }
    let Some((start, end, opener)) = best else {
        return (text, None);
    };
    let main_end = if opener.is_some() { start - 1 } else { start };
    let mut featured = text[end..].trim();
    if let Some(open) = opener {
        let close = if open == '(' { ')' } else { ']' };
        featured = featured.strip_suffix(close).unwrap_or(featured).trim_end();
    }
    let main = text[..main_end].trim_end();
    if main.is_empty() || featured.is_empty() {
        return (text, None);
    }
    (main, Some(featured))
}

/// The artists a track is credited to, as the native scan finds them.
pub fn artists_of(artist_values: &[String], explicit_artists: &[String]) -> Vec<String> {
    let names = if !explicit_artists.is_empty() {
        explicit_artists.to_vec()
    } else if artist_values.len() > 1 {
        artist_values.to_vec()
    } else {
        artist_values
            .first()
            .map(|one| split_artists(one))
            .unwrap_or_default()
    };
    let mut seen = HashSet::new();
    names
        .into_iter()
        .filter(|name| seen.insert(normalize(name)))
        .collect()
}

/// A track as the page keeps it, after reading the file's tags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibTrack {
    /// `<folder>/<path>`: unique in the library.
    pub key: String,
    /// The folder's id in the page's store.
    pub folder: u32,
    /// Where the file is inside the folder, `/`-separated.
    pub path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    #[serde(default)]
    pub artist_values: Vec<String>,
    #[serde(default)]
    pub explicit_artists: Vec<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<i32>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub duration_ms: Option<u64>,
    pub replaygain: Option<ReplayGainInfo>,
    /// The cover's id in the page's artwork store.
    pub artwork: Option<String>,
    /// The lyrics file beside it, by its path in the folder.
    pub lyrics_path: Option<String>,
    /// The file's size and modification time when it was read, to notice changes.
    pub size: f64,
    pub modified_ms: f64,
    /// When it was first added (ms since the epoch).
    pub added_ms: f64,
}

impl LibTrack {
    /// The title, else the file name without its extension.
    fn display_title(&self) -> String {
        if let Some(title) = self.title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
            return title.to_owned();
        }
        let name = self.path.rsplit('/').next().unwrap_or(&self.path);
        match name.rfind('.') {
            Some(dot) if dot > 0 => name[..dot].to_owned(),
            _ => name.to_owned(),
        }
    }
}

/// A track in a list.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackView {
    pub key: String,
    pub title: String,
    pub artist: Option<String>,
    /// The first artist it is credited to.
    pub artist_id: Option<String>,
    pub album: Option<String>,
    pub album_id: Option<String>,
    pub year: Option<i32>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub duration_ms: Option<u64>,
    pub artwork: Option<String>,
}

/// An album in a list.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlbumView {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    /// The artist it is grouped under, when that artist has songs of their own in the library.
    pub artist_id: Option<String>,
    /// The earliest year among its tracks.
    pub year: Option<i32>,
    pub track_count: usize,
    pub duration_ms: u64,
    /// The cover of its first track that has one.
    pub artwork: Option<String>,
}

/// An artist in a list.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArtistView {
    pub id: String,
    pub name: String,
    pub album_count: usize,
    pub track_count: usize,
    pub artwork: Option<String>,
}

/// The library's size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Stats {
    pub tracks: usize,
    pub albums: usize,
    pub artists: usize,
    pub duration_ms: u64,
}

/// What a search found.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchResults {
    pub tracks: Vec<TrackView>,
    pub albums: Vec<AlbumView>,
    pub artists: Vec<ArtistView>,
}

/// How to sort tracks (`jf_library::TrackSort`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackSort {
    Title,
    Artist,
    Album,
    Year,
    Duration,
    Path,
    Added,
}

/// How to sort albums (`jf_library::AlbumSort`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlbumSort {
    Title,
    Artist,
    Year,
    Added,
}

#[derive(Debug)]
struct Norms {
    title: String,
    artist: String,
    album: String,
}

#[derive(Debug)]
struct Album {
    id: String,
    title: String,
    artist: Option<String>,
    title_norm: String,
    artist_norm: String,
    /// Track indexes, in library order.
    tracks: Vec<usize>,
}

#[derive(Debug)]
struct Artist {
    id: String,
    name: String,
    norm: String,
    tracks: Vec<usize>,
}

#[derive(Debug)]
struct Index {
    norms: Vec<Norms>,
    albums: Vec<Album>,
    album_of: Vec<Option<usize>>,
    artists: Vec<Artist>,
    /// Each track's first credited artist.
    artist_of: Vec<Option<usize>>,
    artist_ids: HashMap<String, usize>,
}

/// The catalogue.
#[derive(Debug, Default)]
pub struct Library {
    tracks: Vec<LibTrack>,
    index: Option<Index>,
}

impl Library {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces every track.
    pub fn load(&mut self, mut tracks: Vec<LibTrack>) {
        tracks.sort_by(|a, b| a.key.cmp(&b.key));
        tracks.dedup_by(|a, b| a.key == b.key);
        self.tracks = tracks;
        self.index = None;
    }

    /// Adds tracks, replacing those with the same key.
    pub fn upsert(&mut self, tracks: Vec<LibTrack>) {
        let mut by_key: HashMap<String, LibTrack> =
            self.tracks.drain(..).map(|t| (t.key.clone(), t)).collect();
        for track in tracks {
            by_key.insert(track.key.clone(), track);
        }
        self.load(by_key.into_values().collect());
    }

    /// Removes the tracks with these keys.
    pub fn remove(&mut self, keys: &[String]) {
        let keys: HashSet<&str> = keys.iter().map(String::as_str).collect();
        self.tracks.retain(|t| !keys.contains(t.key.as_str()));
        self.index = None;
    }

    /// Removes every track of a folder.
    pub fn remove_folder(&mut self, folder: u32) {
        self.tracks.retain(|t| t.folder != folder);
        self.index = None;
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    fn index(&mut self) -> &Index {
        if self.index.is_none() {
            self.index = Some(build(&self.tracks));
        }
        self.index.as_ref().expect("built above")
    }

    pub fn stats(&mut self) -> Stats {
        let index = self.index();
        let (albums, artists) = (index.albums.len(), index.artists.len());
        Stats {
            tracks: self.tracks.len(),
            albums,
            artists,
            duration_ms: self.tracks.iter().filter_map(|t| t.duration_ms).sum(),
        }
    }

    fn track_view(&self, index: &Index, i: usize) -> TrackView {
        let track = &self.tracks[i];
        TrackView {
            key: track.key.clone(),
            title: track.display_title(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            artist_id: index.artist_of[i].map(|a| index.artists[a].id.clone()),
            album_id: index.album_of[i].map(|a| index.albums[a].id.clone()),
            year: track.year,
            track_no: track.track_no,
            disc_no: track.disc_no,
            duration_ms: track.duration_ms,
            artwork: track.artwork.clone(),
        }
    }

    fn album_view(&self, index: &Index, album: &Album) -> AlbumView {
        let tracks = || album.tracks.iter().map(|&i| &self.tracks[i]);
        let mut by_position: Vec<usize> = album.tracks.clone();
        by_position.sort_by(|&a, &b| album_order(&self.tracks, a, b));
        AlbumView {
            id: album.id.clone(),
            title: album.title.clone(),
            artist: album.artist.clone(),
            artist_id: index
                .artist_ids
                .contains_key(&album.artist_norm)
                .then(|| album.artist_norm.clone()),
            year: tracks().filter_map(|t| t.year).min(),
            track_count: album.tracks.len(),
            duration_ms: tracks().filter_map(|t| t.duration_ms).sum(),
            artwork: by_position.iter().find_map(|&i| self.tracks[i].artwork.clone()),
        }
    }

    fn artist_view(&self, index: &Index, artist: &Artist) -> ArtistView {
        let albums = self.artist_album_indexes(index, artist);
        ArtistView {
            id: artist.id.clone(),
            name: artist.name.clone(),
            album_count: albums.len(),
            track_count: artist.tracks.len(),
            artwork: albums
                .iter()
                .find_map(|&a| self.album_view(index, &index.albums[a]).artwork)
                .or_else(|| artist.tracks.iter().find_map(|&i| self.tracks[i].artwork.clone())),
        }
    }

    /// Albums with a track credited to the artist, or grouped under the artist's name; by year
    /// (unknown last), then title.
    fn artist_album_indexes(&self, index: &Index, artist: &Artist) -> Vec<usize> {
        let mut set: BTreeSet<usize> = artist.tracks.iter().filter_map(|&i| index.album_of[i]).collect();
        for (a, album) in index.albums.iter().enumerate() {
            if album.artist_norm == artist.norm {
                set.insert(a);
            }
        }
        let years: HashMap<usize, Option<i32>> = set
            .iter()
            .map(|&a| {
                let year = index.albums[a]
                    .tracks
                    .iter()
                    .filter_map(|&i| self.tracks[i].year)
                    .min();
                (a, year)
            })
            .collect();
        let mut list: Vec<usize> = set.into_iter().collect();
        list.sort_by(|&a, &b| {
            nulls_last(years[&a], years[&b], false)
                .then_with(|| index.albums[a].title_norm.cmp(&index.albums[b].title_norm))
                .then(a.cmp(&b))
        });
        list
    }

    /// A page of albums.
    pub fn albums(
        &mut self,
        sort: AlbumSort,
        descending: bool,
        offset: usize,
        limit: usize,
    ) -> Vec<AlbumView> {
        self.index();
        let index = self.index.as_ref().expect("built above");
        let year = |a: usize| {
            index.albums[a]
                .tracks
                .iter()
                .filter_map(|&i| self.tracks[i].year)
                .min()
        };
        let added = |a: usize| {
            index.albums[a]
                .tracks
                .iter()
                .map(|&i| self.tracks[i].added_ms)
                .fold(f64::INFINITY, f64::min)
        };
        let dir = |o: Ordering| if descending { o.reverse() } else { o };
        let mut order: Vec<usize> = (0..index.albums.len()).collect();
        order.sort_by(|&a, &b| {
            let (x, y) = (&index.albums[a], &index.albums[b]);
            match sort {
                AlbumSort::Title => dir(x.title_norm.cmp(&y.title_norm))
                    .then_with(|| dir(x.artist_norm.cmp(&y.artist_norm)))
                    .then_with(|| dir(a.cmp(&b))),
                AlbumSort::Artist => x
                    .artist_norm
                    .is_empty()
                    .cmp(&y.artist_norm.is_empty())
                    .then_with(|| dir(x.artist_norm.cmp(&y.artist_norm)))
                    .then_with(|| nulls_last(year(a), year(b), descending))
                    .then_with(|| dir(x.title_norm.cmp(&y.title_norm)))
                    .then_with(|| dir(a.cmp(&b))),
                AlbumSort::Year => nulls_last(year(a), year(b), descending)
                    .then_with(|| dir(x.title_norm.cmp(&y.title_norm)))
                    .then_with(|| dir(x.artist_norm.cmp(&y.artist_norm)))
                    .then_with(|| dir(a.cmp(&b))),
                AlbumSort::Added => dir(added(a).total_cmp(&added(b))).then_with(|| dir(a.cmp(&b))),
            }
        });
        order
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|a| self.album_view(index, &index.albums[a]))
            .collect()
    }

    /// An album and its tracks in disc and track order.
    pub fn album(&mut self, id: &str) -> Option<(AlbumView, Vec<TrackView>)> {
        self.index();
        let index = self.index.as_ref().expect("built above");
        let album = index.albums.iter().find(|a| a.id == id)?;
        let mut tracks = album.tracks.clone();
        tracks.sort_by(|&a, &b| album_order(&self.tracks, a, b));
        Some((
            self.album_view(index, album),
            tracks.into_iter().map(|i| self.track_view(index, i)).collect(),
        ))
    }

    /// A page of artists, by name.
    pub fn artists(&mut self, offset: usize, limit: usize) -> Vec<ArtistView> {
        self.index();
        let index = self.index.as_ref().expect("built above");
        let mut order: Vec<usize> = (0..index.artists.len()).collect();
        order.sort_by(|&a, &b| index.artists[a].norm.cmp(&index.artists[b].norm).then(a.cmp(&b)));
        order
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|a| self.artist_view(index, &index.artists[a]))
            .collect()
    }

    /// An artist, their albums (by year) and their tracks (by album, disc and track).
    pub fn artist(&mut self, id: &str) -> Option<(ArtistView, Vec<AlbumView>, Vec<TrackView>)> {
        self.index();
        let index = self.index.as_ref().expect("built above");
        let artist = index.artists.iter().find(|a| a.id == id)?;
        let albums = self
            .artist_album_indexes(index, artist)
            .into_iter()
            .map(|a| self.album_view(index, &index.albums[a]))
            .collect();
        let mut tracks = artist.tracks.clone();
        tracks.sort_by(|&a, &b| {
            index.norms[a]
                .album
                .cmp(&index.norms[b].album)
                .then_with(|| album_order(&self.tracks, a, b))
        });
        Some((
            self.artist_view(index, artist),
            albums,
            tracks.into_iter().map(|i| self.track_view(index, i)).collect(),
        ))
    }

    /// A page of tracks.
    pub fn tracks(
        &mut self,
        sort: TrackSort,
        descending: bool,
        offset: usize,
        limit: usize,
    ) -> Vec<TrackView> {
        self.index();
        let index = self.index.as_ref().expect("built above");
        let t = &self.tracks;
        let n = &index.norms;
        let dir = |o: Ordering| if descending { o.reverse() } else { o };
        let rest = |a: usize, b: usize, from_album: bool| {
            let album = if from_album {
                dir(n[a].album.cmp(&n[b].album))
            } else {
                Ordering::Equal
            };
            album
                .then_with(|| dir(t[a].disc_no.cmp(&t[b].disc_no)))
                .then_with(|| dir(t[a].track_no.cmp(&t[b].track_no)))
                .then_with(|| dir(n[a].title.cmp(&n[b].title)))
                .then_with(|| dir(a.cmp(&b)))
        };
        let mut order: Vec<usize> = (0..t.len()).collect();
        order.sort_by(|&a, &b| match sort {
            TrackSort::Title => dir(n[a].title.cmp(&n[b].title)).then_with(|| dir(a.cmp(&b))),
            TrackSort::Artist => t[a]
                .artist
                .is_none()
                .cmp(&t[b].artist.is_none())
                .then_with(|| dir(n[a].artist.cmp(&n[b].artist)))
                .then_with(|| rest(a, b, true)),
            TrackSort::Album => t[a]
                .album
                .is_none()
                .cmp(&t[b].album.is_none())
                .then_with(|| dir(n[a].album.cmp(&n[b].album)))
                .then_with(|| dir(index.album_of[a].cmp(&index.album_of[b])))
                .then_with(|| rest(a, b, false)),
            TrackSort::Year => t[a]
                .year
                .is_none()
                .cmp(&t[b].year.is_none())
                .then_with(|| dir(t[a].year.cmp(&t[b].year)))
                .then_with(|| rest(a, b, true)),
            TrackSort::Duration => t[a]
                .duration_ms
                .is_none()
                .cmp(&t[b].duration_ms.is_none())
                .then_with(|| dir(t[a].duration_ms.cmp(&t[b].duration_ms)))
                .then_with(|| dir(a.cmp(&b))),
            TrackSort::Path => dir((t[a].folder, &t[a].path).cmp(&(t[b].folder, &t[b].path))),
            TrackSort::Added => dir(t[a].added_ms.total_cmp(&t[b].added_ms)).then_with(|| dir(a.cmp(&b))),
        });
        order
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|i| self.track_view(index, i))
            .collect()
    }

    /// The keys of every track, in `sort` order (to play the whole list).
    pub fn track_keys(&mut self, sort: TrackSort, descending: bool) -> Vec<String> {
        let len = self.tracks.len();
        self.tracks(sort, descending, 0, len)
            .into_iter()
            .map(|t| t.key)
            .collect()
    }

    /// Up to `limit` tracks, albums and artists matching `text`.
    pub fn search(&mut self, text: &str, limit: usize) -> SearchResults {
        let query = normalize(text);
        let empty = SearchResults {
            tracks: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
        };
        if query.is_empty() {
            return empty;
        }
        let pieces: Vec<&str> = query.split(' ').collect();
        self.index();
        let index = self.index.as_ref().expect("built above");
        let tracks = rank(
            (0..self.tracks.len()).map(|i| {
                let n = &index.norms[i];
                (i, [n.title.as_str(), n.artist.as_str(), n.album.as_str()])
            }),
            &query,
            &pieces,
            limit,
        );
        let albums = rank(
            index
                .albums
                .iter()
                .enumerate()
                .map(|(a, album)| (a, [album.title_norm.as_str(), album.artist_norm.as_str(), ""])),
            &query,
            &pieces,
            limit,
        );
        let artists = rank(
            index
                .artists
                .iter()
                .enumerate()
                .map(|(a, artist)| (a, [artist.norm.as_str(), "", ""])),
            &query,
            &pieces,
            limit,
        );
        SearchResults {
            tracks: tracks.into_iter().map(|i| self.track_view(index, i)).collect(),
            albums: albums
                .into_iter()
                .map(|a| self.album_view(index, &index.albums[a]))
                .collect(),
            artists: artists
                .into_iter()
                .map(|a| self.artist_view(index, &index.artists[a]))
                .collect(),
        }
    }
}

/// Disc (unknown as 1), then track number (unknown last), title and library order: the order
/// of an album's tracks.
fn album_order(tracks: &[LibTrack], a: usize, b: usize) -> Ordering {
    let (x, y) = (&tracks[a], &tracks[b]);
    x.disc_no
        .unwrap_or(1)
        .cmp(&y.disc_no.unwrap_or(1))
        .then_with(|| x.track_no.is_none().cmp(&y.track_no.is_none()))
        .then_with(|| x.track_no.cmp(&y.track_no))
        .then_with(|| normalize(&x.display_title()).cmp(&normalize(&y.display_title())))
        .then(a.cmp(&b))
}

/// Unknown values last in both directions; known ones by `descending`.
fn nulls_last(a: Option<i32>, b: Option<i32>, descending: bool) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) if descending => y.cmp(&x),
        (Some(x), Some(y)) => x.cmp(&y),
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
    }
}

/// Search tiers over normalised columns (the first is the name): exact name, name prefix, then
/// every piece in some column, a name match first.
fn rank<'a>(
    rows: impl Iterator<Item = (usize, [&'a str; 3])>,
    query: &str,
    pieces: &[&str],
    limit: usize,
) -> Vec<usize> {
    let mut found: Vec<(u8, usize)> = rows
        .filter_map(|(i, columns)| {
            let [name, second, _] = columns;
            let tier = if name == query {
                0
            } else if name.starts_with(query) {
                1
            } else if !pieces.iter().all(|p| columns.iter().any(|c| c.contains(p))) {
                return None;
            } else if pieces.iter().all(|p| name.contains(p)) {
                2
            } else if pieces.iter().all(|p| name.contains(p) || second.contains(p)) {
                3
            } else {
                4
            };
            Some((tier, i))
        })
        .collect();
    found.sort();
    found.into_iter().take(limit).map(|(_, i)| i).collect()
}

fn build(tracks: &[LibTrack]) -> Index {
    let mut norms = Vec::with_capacity(tracks.len());
    let mut albums: Vec<Album> = Vec::new();
    let mut album_ids: HashMap<(String, String), usize> = HashMap::new();
    let mut album_of = Vec::with_capacity(tracks.len());
    let mut artists: Vec<Artist> = Vec::new();
    let mut artist_ids: HashMap<String, usize> = HashMap::new();
    let mut artist_of = Vec::with_capacity(tracks.len());
    for (i, track) in tracks.iter().enumerate() {
        norms.push(Norms {
            title: normalize(&track.display_title()),
            artist: normalize(track.artist.as_deref().unwrap_or_default()),
            album: normalize(track.album.as_deref().unwrap_or_default()),
        });
        let credited = artists_of(&track.artist_values, &track.explicit_artists);
        // Grouped by album artist, else the primary track artist.
        let group_artist = track
            .album_artist
            .clone()
            .or_else(|| credited.first().cloned())
            .or_else(|| track.artist.clone());
        let album = track.album.as_deref().and_then(|title| {
            let key = (
                normalize(title),
                normalize(group_artist.as_deref().unwrap_or_default()),
            );
            if key.0.is_empty() {
                return None;
            }
            let next = albums.len();
            let a = *album_ids.entry(key.clone()).or_insert(next);
            if a == next {
                albums.push(Album {
                    id: format!("{}\u{1f}{}", key.0, key.1),
                    title: title.to_owned(),
                    artist: group_artist.clone(),
                    title_norm: key.0,
                    artist_norm: key.1,
                    tracks: Vec::new(),
                });
            }
            albums[a].tracks.push(i);
            Some(a)
        });
        album_of.push(album);
        let mut mine = Vec::new();
        for name in credited {
            let norm = normalize(&name);
            if norm.is_empty() {
                continue;
            }
            let next = artists.len();
            let a = *artist_ids.entry(norm.clone()).or_insert(next);
            if a == next {
                artists.push(Artist {
                    id: norm.clone(),
                    name,
                    norm,
                    tracks: Vec::new(),
                });
            }
            if !mine.contains(&a) {
                artists[a].tracks.push(i);
                mine.push(a);
            }
        }
        artist_of.push(mine.first().copied());
    }
    Index {
        norms,
        albums,
        album_of,
        artists,
        artist_of,
        artist_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(key: &str, title: &str, artist: &str, album: Option<&str>) -> LibTrack {
        LibTrack {
            key: key.into(),
            folder: 1,
            path: key.trim_start_matches("1/").into(),
            title: Some(title.into()),
            artist: Some(artist.into()),
            artist_values: vec![artist.into()],
            explicit_artists: Vec::new(),
            album: album.map(Into::into),
            album_artist: None,
            year: None,
            track_no: None,
            disc_no: None,
            duration_ms: Some(1000),
            replaygain: None,
            artwork: None,
            lyrics_path: None,
            size: 1.0,
            modified_ms: 0.0,
            added_ms: 0.0,
        }
    }

    #[test]
    fn featured_artists_stay_on_the_main_artists_album() {
        let mut library = Library::new();
        library.load(vec![
            track("1/a.flac", "One", "Aurora", Some("Sky")),
            track("1/b.flac", "Two", "Aurora feat. Bo", Some("Sky")),
            track("1/c.flac", "Three", "Bo", Some("Sky")),
        ]);
        let albums = library.albums(AlbumSort::Title, false, 0, 10);
        assert_eq!(albums.len(), 2, "{albums:?}");
        assert_eq!(albums[0].artist.as_deref(), Some("Aurora"));
        assert_eq!(albums[0].track_count, 2);
        let artists = library.artists(0, 10);
        let names: Vec<&str> = artists.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Aurora", "Bo"]);
        assert_eq!(artists[1].track_count, 2);
        // Bo's albums: the one Bo is featured on and Bo's own.
        assert_eq!(albums[0].artist_id.as_deref(), Some("aurora"));
        let (_, bo_albums, bo_tracks) = library.artist("bo").unwrap();
        let featured = bo_tracks.iter().find(|t| t.title == "Two").unwrap();
        assert_eq!(
            featured.artist_id.as_deref(),
            Some("aurora"),
            "credited to Aurora first"
        );
        assert_eq!(bo_albums.len(), 2);
        assert_eq!(bo_tracks.len(), 2);
        let stats = library.stats();
        assert_eq!(
            (stats.tracks, stats.albums, stats.artists, stats.duration_ms),
            (3, 2, 2, 3000)
        );
    }

    #[test]
    fn unknown_values_sort_last_both_ways() {
        let mut library = Library::new();
        let mut a = track("1/a.flac", "Alpha", "Zed", Some("A"));
        a.year = Some(1999);
        let mut b = track("1/b.flac", "Beta", "Ann", Some("B"));
        b.year = Some(2005);
        let mut c = track("1/c.flac", "Gamma", "Cy", None);
        c.artist = None;
        c.artist_values.clear();
        library.load(vec![a, b, c]);
        let titles = |list: Vec<TrackView>| list.into_iter().map(|t| t.title).collect::<Vec<_>>();
        assert_eq!(
            titles(library.tracks(TrackSort::Artist, false, 0, 10)),
            ["Beta", "Alpha", "Gamma"]
        );
        assert_eq!(
            titles(library.tracks(TrackSort::Artist, true, 0, 10)),
            ["Alpha", "Beta", "Gamma"]
        );
        assert_eq!(
            titles(library.tracks(TrackSort::Year, true, 0, 10)),
            ["Beta", "Alpha", "Gamma"]
        );
        let albums = library.albums(AlbumSort::Year, true, 0, 10);
        assert_eq!(albums[0].title, "B");
        assert_eq!(
            library.track_keys(TrackSort::Title, false),
            ["1/a.flac", "1/b.flac", "1/c.flac"]
        );
    }

    #[test]
    fn albums_play_in_disc_and_track_order() {
        let mut library = Library::new();
        let mut tracks = Vec::new();
        for (key, disc, no) in [
            ("1/d.flac", Some(2), Some(1)),
            ("1/a.flac", None, Some(2)),
            ("1/b.flac", Some(1), Some(1)),
        ] {
            let mut t = track(key, key, "Aurora", Some("Sky"));
            t.disc_no = disc;
            t.track_no = no;
            tracks.push(t);
        }
        library.load(tracks);
        let id = library.albums(AlbumSort::Title, false, 0, 1)[0].id.clone();
        let (_, list) = library.album(&id).unwrap();
        let keys: Vec<&str> = list.iter().map(|t| t.key.as_str()).collect();
        assert_eq!(keys, ["1/b.flac", "1/a.flac", "1/d.flac"]);
    }

    #[test]
    fn search_ranks_exact_then_prefix_then_anywhere() {
        let mut library = Library::new();
        library.load(vec![
            track("1/a.flac", "Moonlight Drive", "Doors", Some("Strange Days")),
            track("1/b.flac", "Moon", "Sigur Rós", Some("Ágætis byrjun")),
            track("1/c.flac", "Blue Moon", "Billie", None),
            track("1/d.flac", "Song", "Moon Band", None),
        ]);
        let found = library.search("MOON", 10);
        let titles: Vec<&str> = found.tracks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["Moon", "Moonlight Drive", "Blue Moon", "Song"]);
        assert_eq!(library.search("sigur ros", 10).artists[0].name, "Sigur Rós");
        assert_eq!(library.search("ÁGÆTIS", 10).albums.len(), 1);
        assert!(library.search("   ", 10).tracks.is_empty());
    }

    #[test]
    fn upserts_replace_and_removals_forget() {
        let mut library = Library::new();
        library.load(vec![track("1/a.flac", "Old", "A", None)]);
        let mut other = track("2/b.flac", "B", "A", None);
        other.folder = 2;
        library.upsert(vec![track("1/a.flac", "New", "A", None), other]);
        assert_eq!(library.len(), 2);
        assert_eq!(library.tracks(TrackSort::Path, false, 0, 1)[0].title, "New");
        library.remove(&["1/a.flac".into()]);
        assert_eq!(library.len(), 1);
        library.remove_folder(2);
        assert!(library.is_empty());
    }

    #[test]
    fn untitled_tracks_show_their_file_name() {
        let mut t = track("1/x/Some Song.mp3", "", "A", None);
        t.title = None;
        assert_eq!(t.display_title(), "Some Song");
    }
}
