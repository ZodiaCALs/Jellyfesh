//! Search, the library, and the details behind them.
//!
//! * Search: `weapi/cloudsearch/get/web` once per kind (`type` 1 songs, 10
//!   albums, 100 artists, 1000 playlists). Songs come with the account's
//!   privileges.
//! * Library: the liked songs (`weapi/song/like/get`, ids), the account's own
//!   and saved playlists (`weapi/user/playlist`).
//! * A playlist's songs: its `trackIds` (`weapi/v6/playlist/detail`), then
//!   the songs of one page with their privileges (`weapi/v3/song/detail`).
//! * An album's songs: `weapi/v1/album/<id>`. An artist's albums:
//!   `weapi/artist/albums/<id>`.

use jf_community_common::matching::{self, Candidate};
use jf_community_common::paging;
use jf_plugin_sdk::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api;
use crate::items::{self, Id};
use crate::model::{Album, Artist, Playlist, Privilege, Song};

/// The search kinds in the order results are listed: (kind, `type`, result
/// list, result count).
const KINDS: [(ItemKind, u32, &str, &str); 4] = [
    (ItemKind::Artist, 100, "artists", "artistCount"),
    (ItemKind::Album, 10, "albums", "albumCount"),
    (ItemKind::Playlist, 1000, "playlists", "playlistCount"),
    (ItemKind::Track, 1, "songs", "songCount"),
];
/// The track kind's index in [`KINDS`].
const TRACKS: usize = 3;

/// What NetEase sends outside mainland China instead of search results.
const SCRAMBLED_RESULTS: &str = "NetEase Cloud Music sent its search results in the scrambled form it uses outside \
     mainland China, which this plugin does not read.";

/// Where a search continues: an offset per kind, or done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchCursor {
    offsets: [usize; 4],
    done: [bool; 4],
}

impl SearchCursor {
    /// Reads `"0,10,-,40"` (`-`: no more of that kind).
    pub fn parse(cursor: Option<&str>) -> Result<SearchCursor> {
        let Some(cursor) = cursor.filter(|c| !c.is_empty()) else {
            return Ok(SearchCursor::default());
        };
        let bad = || PluginError::invalid_input(format!("bad cursor {cursor:?}"));
        let parts: Vec<&str> = cursor.split(',').collect();
        if parts.len() != 4 {
            return Err(bad());
        }
        let mut out = SearchCursor::default();
        for (i, part) in parts.iter().enumerate() {
            if *part == "-" {
                out.done[i] = true;
            } else {
                out.offsets[i] = part.parse().map_err(|_| bad())?;
            }
        }
        Ok(out)
    }

    /// Writes the cursor back.
    pub fn encode(&self) -> String {
        (0..4)
            .map(|i| {
                if self.done[i] {
                    "-".to_string()
                } else {
                    self.offsets[i].to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// How many of each kind to ask for: a tenth each for artists, albums and
/// playlists and the rest for songs when songs are wanted, an even share
/// otherwise.
pub fn allocation(wanted: [bool; 4], limit: u32) -> [u32; 4] {
    let count = wanted.iter().filter(|w| **w).count() as u32;
    let mut out = [0u32; 4];
    if count == 0 {
        return out;
    }
    if count == 1 {
        let only = wanted.iter().position(|w| *w).unwrap_or(TRACKS);
        out[only] = limit;
        return out;
    }
    if wanted[TRACKS] {
        let small = limit / 10;
        for i in 0..TRACKS {
            if wanted[i] {
                out[i] = small;
            }
        }
        out[TRACKS] = limit - out.iter().take(TRACKS).sum::<u32>();
        return out;
    }
    let share = limit / count;
    let mut left = limit - share * count;
    for i in 0..4 {
        if wanted[i] {
            out[i] = share + u32::from(left > 0);
            left = left.saturating_sub(1);
        }
    }
    out
}

/// One kind's search answer: the items, how many entries the answer had,
/// and the total, if said.
fn search_kind(
    query: &str,
    kind: usize,
    count: u32,
    offset: usize,
) -> Result<(Vec<Item>, usize, Option<usize>)> {
    let (_, search_type, list_key, count_key) = KINDS[kind];
    let body = api::call(
        "/cloudsearch/get/web",
        json!({
            "s": query,
            "type": search_type,
            "limit": count,
            "offset": offset,
            "total": true,
        }),
    )?;
    let result = body.get("result").cloned().unwrap_or(Value::Null);
    if result.is_string() {
        return Err(PluginError::service(SCRAMBLED_RESULTS));
    }
    let total = result.get(count_key).and_then(Value::as_u64).map(|t| t as usize);
    let list = result.get(list_key).cloned().unwrap_or(Value::Null);
    let entries = list.as_array().map_or(0, Vec::len).min(count as usize);
    let items = match kind {
        0 => list_of::<Artist>(list).iter().filter_map(items::artist).collect(),
        1 => list_of::<Album>(list).iter().filter_map(items::album).collect(),
        2 => list_of::<Playlist>(list)
            .iter()
            .filter_map(items::playlist)
            .collect(),
        _ => items::tracks(&list_of::<Song>(list), &[]),
    };
    Ok((items.into_iter().take(count as usize).collect(), entries, total))
}

/// A JSON list read entry by entry: an entry that does not fit is skipped.
pub fn list_of<T: serde::de::DeserializeOwned>(value: Value) -> Vec<T> {
    match value {
        Value::Array(entries) => entries
            .into_iter()
            .filter_map(|entry| serde_json::from_value(entry).ok())
            .collect(),
        _ => Vec::new(),
    }
}

/// `catalog.search`.
pub fn search(query: &str, filters: &SearchFilters, page: &PageRequest) -> Result<ItemPage> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(ItemPage::new(Vec::new()));
    }
    let wants = |kind| filters.kinds.is_empty() || filters.kinds.contains(&kind);
    let wanted = KINDS.map(|(kind, ..)| wants(kind));
    let mut cursor = SearchCursor::parse(page.cursor.as_deref())?;
    let left = [0, 1, 2, 3].map(|i| wanted[i] && !cursor.done[i]);
    let counts = allocation(left, paging::limit(page) as u32);
    let mut results = Vec::new();
    for i in 0..4 {
        if !wanted[i] {
            cursor.done[i] = true;
            continue;
        }
        if counts[i] == 0 || cursor.done[i] {
            continue;
        }
        let (found, entries, total) = search_kind(query, i, counts[i], cursor.offsets[i])?;
        cursor.offsets[i] += entries;
        cursor.done[i] = entries < counts[i] as usize || total.is_some_and(|t| cursor.offsets[i] >= t);
        results.extend(found);
    }
    let more = (0..4).any(|i| wanted[i] && !cursor.done[i]);
    Ok(ItemPage {
        items: results,
        next: more.then(|| cursor.encode()),
        total: None,
    })
}

/// The root of the library.
fn root() -> ItemPage {
    ItemPage::new(vec![
        items::folder("liked", "Liked Songs"),
        items::folder("playlists", "My Playlists"),
        items::folder("saved", "Saved Playlists"),
    ])
}

/// `catalog.browse`.
pub fn browse(node: Option<&Item>, page: &PageRequest) -> Result<ItemPage> {
    let Some(node) = node else {
        api::require_session()?;
        return Ok(root());
    };
    match Id::parse(&node.id) {
        Some(Id::Folder("liked")) => liked(page),
        Some(Id::Folder(which @ ("playlists" | "saved"))) => {
            let (own, saved) = user_playlists()?;
            paging::slice(if which == "playlists" { own } else { saved }, page)
        }
        Some(Id::Folder(name)) => {
            crate::discover::browse_folder(name, page).unwrap_or(Err(PluginError::NotFound))
        }
        Some(Id::Playlist(id)) => playlist_tracks(id, page),
        Some(Id::Album(id)) => paging::slice(album_tracks(id)?, page),
        Some(Id::Artist(id)) => artist_albums(id, page),
        Some(Id::Track(_)) => Err(PluginError::invalid_input("a track has no children")),
        None => Err(PluginError::NotFound),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LikedIds {
    ids: Vec<Value>,
}

/// The liked songs, newest first.
fn liked(page: &PageRequest) -> Result<ItemPage> {
    let session = api::require_session()?;
    let key = format!("liked:{}", session.user_id);
    let ids = match api::cached_ids(&key) {
        Some(ids) => ids,
        None => {
            let found: LikedIds = api::call_as("/song/like/get", json!({ "uid": session.user_id }))?;
            let ids: Vec<u64> = found
                .ids
                .iter()
                .filter_map(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
                .filter(|id| *id > 0)
                .collect();
            api::keep_ids(&key, &ids);
            ids
        }
    };
    page_of_songs(&ids, page)
}

/// One page of a list of song ids, as tracks.
fn page_of_songs(ids: &[u64], page: &PageRequest) -> Result<ItemPage> {
    let start = paging::offset(page)?;
    let end = start.saturating_add(paging::limit(page)).min(ids.len());
    let chunk = if start < end { &ids[start..end] } else { &[][..] };
    let items = song_details(chunk)?;
    Ok(ItemPage {
        items,
        next: (end < ids.len()).then(|| end.to_string()),
        total: u32::try_from(ids.len()).ok(),
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Details {
    songs: Vec<Value>,
    privileges: Vec<Value>,
}

/// The songs with these ids and their privileges, in the order asked.
pub fn songs(ids: &[u64]) -> Result<(Vec<Song>, Vec<Privilege>)> {
    if ids.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let wanted: Vec<String> = ids.iter().map(|id| format!("{{\"id\":{id}}}")).collect();
    let found: Details = api::call_as(
        "/v3/song/detail",
        json!({ "c": format!("[{}]", wanted.join(",")) }),
    )?;
    let songs = list_of::<Song>(Value::Array(found.songs));
    let privileges = list_of::<Privilege>(Value::Array(found.privileges));
    let ordered = ids
        .iter()
        .filter_map(|id| songs.iter().find(|s| s.id == *id).cloned())
        .collect();
    Ok((ordered, privileges))
}

/// Tracks for song ids, in the order asked.
fn song_details(ids: &[u64]) -> Result<Vec<Item>> {
    let (songs, privileges) = songs(ids)?;
    Ok(items::tracks(&songs, &privileges))
}

/// One song.
pub fn song(id: u64) -> Result<Song> {
    songs(&[id])?.0.into_iter().next().ok_or(PluginError::NotFound)
}

/// The account's own playlists (without the liked songs, which have their
/// own section) and the playlists it saved.
fn user_playlists() -> Result<(Vec<Item>, Vec<Item>)> {
    let session = api::require_session()?;
    let body = api::call(
        "/user/playlist",
        json!({ "uid": session.user_id, "limit": 1000, "offset": 0, "includeVideo": true }),
    )?;
    let lists = list_of::<Playlist>(body.get("playlist").cloned().unwrap_or(Value::Null));
    let mut own = Vec::new();
    let mut saved = Vec::new();
    for list in &lists {
        let creator = list.creator.as_ref().map(|c| c.user_id).unwrap_or(list.user_id);
        let mine = creator == session.user_id && list.subscribed != Some(true);
        if mine && list.special_type == Some(5) {
            continue;
        }
        if let Some(item) = items::playlist(list) {
            if mine {
                own.push(item);
            } else {
                saved.push(item);
            }
        }
    }
    Ok((own, saved))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PlaylistAnswer {
    playlist: Option<Playlist>,
}

/// A playlist with its track ids.
pub fn playlist(id: u64) -> Result<Playlist> {
    let found: PlaylistAnswer =
        api::call_as("/v6/playlist/detail", json!({ "id": id, "n": 100_000, "s": 8 }))?;
    found.playlist.filter(|p| p.id != 0).ok_or(PluginError::NotFound)
}

fn playlist_tracks(id: u64, page: &PageRequest) -> Result<ItemPage> {
    let key = format!("playlist:{id}");
    let ids = match api::cached_ids(&key) {
        Some(ids) => ids,
        None => {
            let found = playlist(id)?;
            api::remember_cover(&items::playlist_id(id), found.cover_img_url.as_deref());
            let mut ids: Vec<u64> = found
                .track_ids
                .iter()
                .map(|t| t.id)
                .filter(|id| *id > 0)
                .collect();
            if ids.is_empty() {
                ids = found.tracks.iter().map(|t| t.id).filter(|id| *id > 0).collect();
            }
            api::keep_ids(&key, &ids);
            ids
        }
    };
    page_of_songs(&ids, page)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AlbumAnswer {
    album: Option<Album>,
    songs: Vec<Value>,
}

/// An album and its songs.
pub fn album(id: u64) -> Result<(Album, Vec<Song>)> {
    let found: AlbumAnswer = api::call_as(&format!("/v1/album/{id}"), json!({}))?;
    let album = found.album.filter(|a| a.id != 0).ok_or(PluginError::NotFound)?;
    Ok((album, list_of::<Song>(Value::Array(found.songs))))
}

/// An album's tracks. When the songs come without privileges, they are
/// asked for in one more request, so nothing unplayable looks playable.
fn album_tracks(id: u64) -> Result<Vec<Item>> {
    let (album, songs) = album(id)?;
    api::remember_cover(&items::album_id(id), album.pic_url.as_deref());
    let missing: Vec<u64> = songs
        .iter()
        .filter(|s| s.privilege.is_none())
        .map(|s| s.id)
        .collect();
    let privileges = if missing.is_empty() {
        Vec::new()
    } else {
        songs_privileges(&missing)?
    };
    Ok(items::tracks(&songs, &privileges))
}

fn songs_privileges(ids: &[u64]) -> Result<Vec<Privilege>> {
    Ok(songs(ids)?.1)
}

fn artist_albums(id: u64, page: &PageRequest) -> Result<ItemPage> {
    let offset = paging::offset(page)?;
    let limit = paging::limit(page);
    let body = api::call(
        &format!("/artist/albums/{id}"),
        json!({ "limit": limit, "offset": offset, "total": true }),
    )?;
    let albums = list_of::<Album>(body.get("hotAlbums").cloned().unwrap_or(Value::Null));
    let more = body.get("more").and_then(Value::as_bool).unwrap_or(false);
    let items: Vec<Item> = albums.iter().filter_map(items::album).collect();
    let got = albums.len();
    Ok(ItemPage {
        items,
        next: (more && got > 0).then(|| (offset + got).to_string()),
        total: None,
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArtistAnswer {
    artist: Option<Artist>,
}

/// An artist's details.
pub fn artist(id: u64) -> Result<Artist> {
    let found: ArtistAnswer = api::call_as(&format!("/v1/artist/{id}"), json!({}))?;
    found.artist.filter(|a| a.id != 0).ok_or(PluginError::NotFound)
}

/// Finds NetEase's own song for an item from another source (see
/// `jf_community_common::matching`).
pub fn find_song(item: &Item) -> Result<Option<Song>> {
    let title = item.title.trim();
    if title.is_empty() {
        return Ok(None);
    }
    let query = match item.artist.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
        Some(artist) => format!("{title} {}", matching::primary_artist(artist)),
        None => title.to_string(),
    };
    let body = api::call(
        "/cloudsearch/get/web",
        json!({ "s": query, "type": 1, "limit": 10, "offset": 0, "total": false }),
    )?;
    let result = body.get("result").cloned().unwrap_or(Value::Null);
    if result.is_string() {
        return Ok(None);
    }
    let found = list_of::<Song>(result.get("songs").cloned().unwrap_or(Value::Null));
    let artists: Vec<Option<String>> = found
        .iter()
        .map(|s| jf_community_common::join_artists(s.ar.iter().filter_map(|a| a.name.as_deref())))
        .collect();
    let paired: Vec<(&Song, Option<&str>)> = found.iter().zip(artists.iter().map(|a| a.as_deref())).collect();
    Ok(matching::best(item, &paired, |(song, artist)| Candidate {
        title: song.name.as_deref().unwrap_or_default(),
        artist: *artist,
        album: song.al.as_ref().and_then(|a| a.name.as_deref()),
        duration_ms: song.dt.and_then(|d| u32::try_from(d).ok()),
    })
    .map(|(song, _)| (*song).clone()))
}

/// Finds NetEase's own album for an album item from another source.
pub fn find_album(item: &Item) -> Result<Option<Album>> {
    let title = item.title.trim();
    if title.is_empty() {
        return Ok(None);
    }
    let body = api::call(
        "/cloudsearch/get/web",
        json!({ "s": title, "type": 10, "limit": 10, "offset": 0, "total": false }),
    )?;
    let result = body.get("result").cloned().unwrap_or(Value::Null);
    if result.is_string() {
        return Ok(None);
    }
    let found = list_of::<Album>(result.get("albums").cloned().unwrap_or(Value::Null));
    let artists: Vec<Option<String>> = found
        .iter()
        .map(|a| {
            let list: Vec<&Artist> = if a.artists.is_empty() {
                a.artist.iter().collect()
            } else {
                a.artists.iter().collect()
            };
            jf_community_common::join_artists(list.iter().filter_map(|x| x.name.as_deref()))
        })
        .collect();
    let paired: Vec<(&Album, Option<&str>)> =
        found.iter().zip(artists.iter().map(|a| a.as_deref())).collect();
    let mut wanted = item.clone();
    wanted.duration_ms = None;
    Ok(matching::best(&wanted, &paired, |(album, artist)| Candidate {
        title: album.name.as_deref().unwrap_or_default(),
        artist: *artist,
        album: None,
        duration_ms: None,
    })
    .map(|(album, _)| (*album).clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_cursors_round_trip() {
        let cursor = SearchCursor::parse(Some("4,-,0,20")).unwrap();
        assert_eq!(cursor.offsets, [4, 0, 0, 20]);
        assert_eq!(cursor.done, [false, true, false, false]);
        assert_eq!(cursor.encode(), "4,-,0,20");
        assert_eq!(SearchCursor::parse(None).unwrap(), SearchCursor::default());
        assert!(SearchCursor::parse(Some("1,2,3")).is_err());
        assert!(SearchCursor::parse(Some("a,b,c,d")).is_err());
    }

    #[test]
    fn shares_a_page_between_kinds() {
        assert_eq!(allocation([true; 4], 50), [5, 5, 5, 35]);
        assert_eq!(allocation([false, false, false, true], 50), [0, 0, 0, 50]);
        assert_eq!(allocation([true, false, false, false], 7), [7, 0, 0, 0]);
        assert_eq!(allocation([true, true, false, false], 7), [4, 3, 0, 0]);
        assert_eq!(allocation([true, true, true, false], 10), [4, 3, 3, 0]);
        assert_eq!(allocation([true; 4], 5), [0, 0, 0, 5]);
        assert_eq!(allocation([false; 4], 9), [0; 4]);
    }
}
