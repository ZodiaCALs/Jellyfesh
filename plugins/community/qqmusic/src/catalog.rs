//! Search, the library, and the details behind them. Each is one call on
//! the web player's API:
//!
//! * search: `music.search.SearchCgiService` `DoSearchForQQMusicDesktop`
//!   once per kind (`search_type` 0 songs, 1 singers, 2 albums, 3
//!   playlists), paged by the service;
//! * the account's playlists, its liked songs among them (folder 201):
//!   `music.musicasset.PlaylistBaseRead` `GetPlaylistByUin`;
//! * a playlist's songs: `music.srfDissInfo.aiDissInfo` `uniform_get_Dissinfo`;
//! * an album's songs: `music.musichallAlbum.AlbumSongList` `GetAlbumSongList`;
//! * a singer's albums: `music.musichallAlbum.AlbumListServer` `GetAlbumList`;
//! * one song: `music.pf_song_detail_svr` `get_song_detail_yqq`.

use jf_community_common::matching::{self, Candidate};
use jf_community_common::paging;
use jf_plugin_sdk::prelude::*;
use serde_json::{Value, json};

use crate::api::{self, OwnPlaylist};
use crate::items::{self, Id};
use crate::model::{self, Track, number, text};

/// The search kinds in the order results are listed: (kind, `search_type`,
/// key in the answer's `body`).
const KINDS: [(ItemKind, u32, &str); 4] = [
    (ItemKind::Artist, 1, "singer"),
    (ItemKind::Album, 2, "album"),
    (ItemKind::Playlist, 3, "songlist"),
    (ItemKind::Track, 0, "song"),
];
/// The track kind's index in [`KINDS`].
const TRACKS: usize = 3;
/// The folder number of the liked songs among the account's playlists.
const LIKED_FOLDER: u64 = 201;

/// Where a search continues: per kind, the next page and its size (the
/// service pages by number, so a kind keeps the size it started with), or
/// done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchCursor {
    next: [Option<(u32, u32)>; 4],
    done: [bool; 4],
}

impl SearchCursor {
    /// Reads `"2x5,-,0,3x40"`: page 2 of 5, done, not started, page 3 of 40.
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
            match *part {
                "-" => out.done[i] = true,
                "0" => {}
                _ => {
                    let (page, size) = part.split_once('x').ok_or_else(bad)?;
                    let page: u32 = page.parse().map_err(|_| bad())?;
                    let size: u32 = size.parse().map_err(|_| bad())?;
                    if page == 0 || size == 0 || size > paging::MAX_PAGE {
                        return Err(bad());
                    }
                    out.next[i] = Some((page, size));
                }
            }
        }
        Ok(out)
    }

    /// Writes the cursor back.
    pub fn encode(&self) -> String {
        (0..4)
            .map(|i| match (self.done[i], self.next[i]) {
                (true, _) => "-".to_string(),
                (false, None) => "0".to_string(),
                (false, Some((page, size))) => format!("{page}x{size}"),
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// How many of each kind to ask for (as in the NetEase plugin): a tenth each
/// for singers, albums and playlists and the rest for songs when songs are
/// wanted, an even share otherwise.
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

/// One search page of one kind: its entries and the total, if said.
fn search_page(query: &str, kind: usize, page: u32, size: u32) -> Result<(Vec<Value>, Option<u64>)> {
    let (_, search_type, key) = KINDS[kind];
    let search_id = format!(
        "{}{:06}",
        clock::now_utc_ms(),
        crypto::random_bytes(2).map_or(0, |b| u32::from(b[0]) * 256 + u32::from(b[1]))
    );
    let data = api::call(
        "music.search.SearchCgiService",
        "DoSearchForQQMusicDesktop",
        json!({
            "query": query,
            "search_type": search_type,
            "page_num": page,
            "num_per_page": size,
            "grp": 1,
            "remoteplace": "txt.yqq.center",
            "searchid": search_id,
        }),
    )?;
    let entries = data
        .get("body")
        .and_then(|b| b.get(key))
        .map(|k| model::list(k, "list").to_vec())
        .unwrap_or_default();
    let total = data.get("meta").and_then(|m| number(m, &["sum", "total"]));
    Ok((entries, total))
}

fn to_items(kind: usize, entries: &[Value]) -> Vec<Item> {
    match kind {
        0 => entries.iter().filter_map(items::artist).collect(),
        1 => entries.iter().filter_map(items::album).collect(),
        2 => entries.iter().filter_map(items::playlist).collect(),
        _ => items::tracks(&model::tracks(&Value::Array(entries.to_vec()))),
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
    let limit = paging::limit(page) as u32;
    let left = [0, 1, 2, 3].map(|i| wanted[i] && !cursor.done[i]);
    let shares = allocation(left, limit);
    let mut budget = limit;
    let mut results = Vec::new();
    for i in 0..4 {
        if !wanted[i] {
            cursor.done[i] = true;
            continue;
        }
        if cursor.done[i] {
            continue;
        }
        let (number, size) = match cursor.next[i] {
            Some(next) => next,
            None if shares[i] > 0 => (1, shares[i]),
            None => continue,
        };
        if size > budget {
            continue;
        }
        let (entries, total) = search_page(query, i, number, size)?;
        let found: Vec<Item> = to_items(i, &entries).into_iter().take(size as usize).collect();
        budget -= found.len().min(size as usize) as u32;
        results.extend(found);
        let seen = u64::from(number) * u64::from(size);
        if entries.len() < size as usize || total.is_some_and(|t| seen >= t) {
            cursor.done[i] = true;
        } else {
            cursor.next[i] = Some((number + 1, size));
        }
    }
    let more = (0..4).any(|i| wanted[i] && !cursor.done[i]);
    Ok(ItemPage {
        items: results,
        next: more.then(|| cursor.encode()),
        total: None,
    })
}

fn root() -> ItemPage {
    ItemPage::new(vec![
        items::folder("liked", "Liked Songs"),
        items::folder("playlists", "My Playlists"),
    ])
}

/// `catalog.browse`.
pub fn browse(node: Option<&Item>, page: &PageRequest) -> Result<ItemPage> {
    let Some(node) = node else {
        api::require_session()?;
        return Ok(root());
    };
    match Id::parse(&node.id) {
        Some(Id::Folder("liked")) => {
            let liked = own_playlists()?
                .into_iter()
                .find(|p| p.liked)
                .ok_or(PluginError::NotFound)?;
            playlist_page(&liked.id, page)
        }
        Some(Id::Folder("playlists")) => {
            let lists: Vec<Item> = own_playlists()?
                .iter()
                .filter(|p| !p.liked)
                .map(items::own_playlist)
                .collect();
            paging::slice(lists, page)
        }
        Some(Id::Folder(_)) => Err(PluginError::NotFound),
        Some(Id::Playlist(id)) => playlist_page(id, page),
        Some(Id::Album(mid)) => album_page(mid, page),
        Some(Id::Artist(mid)) => artist_albums(mid, page),
        Some(Id::Track(_)) => Err(PluginError::invalid_input("a track has no children")),
        None => Err(PluginError::NotFound),
    }
}

/// The account's own playlists, its liked songs (folder 201) among them.
fn own_playlists() -> Result<Vec<OwnPlaylist>> {
    if let Some(lists) = api::cached_playlists() {
        return Ok(lists);
    }
    let session = api::require_session()?;
    let data = api::call_signed_in(
        "music.musicasset.PlaylistBaseRead",
        "GetPlaylistByUin",
        json!({ "uin": session.musicid.to_string() }),
    )?;
    let entries = ["v_playlist", "playlist", "list", "v_list"]
        .iter()
        .map(|key| model::list(&data, key))
        .find(|list| !list.is_empty())
        .unwrap_or(&[]);
    let lists: Vec<OwnPlaylist> = entries
        .iter()
        .filter_map(|entry| {
            let id = text(entry, &["tid", "dissid", "disstid", "id"])?;
            if !id.bytes().all(|b| b.is_ascii_digit()) || id == "0" {
                return None;
            }
            let liked = number(entry, &["dirId", "dirid", "dir_id"]) == Some(LIKED_FOLDER);
            Some(OwnPlaylist {
                title: text(entry, &["dirName", "diss_name", "dissname", "title", "name"])
                    .map(|t| jf_community_common::entities::decode(&t))
                    .unwrap_or_else(|| {
                        if liked {
                            "Liked Songs".into()
                        } else {
                            format!("Playlist {id}")
                        }
                    }),
                cover: text(entry, &["picUrl", "diss_cover", "picurl", "logo", "cover"]),
                count: number(entry, &["songNum", "song_cnt", "songnum", "song_num"])
                    .and_then(|n| u32::try_from(n).ok()),
                liked,
                id,
            })
        })
        .collect();
    api::keep_playlists(&lists);
    Ok(lists)
}

/// A page of a playlist's songs (the service pages them).
fn playlist_page(id: &str, page: &PageRequest) -> Result<ItemPage> {
    let offset = paging::offset(page)?;
    let limit = paging::limit(page);
    let data = api::call(
        "music.srfDissInfo.aiDissInfo",
        "uniform_get_Dissinfo",
        json!({
            "disstid": id.parse::<u64>().unwrap_or(0),
            "userinfo": 1,
            "tag": 1,
            "orderlist": 1,
            "song_begin": offset,
            "song_num": limit,
            "onlysonglist": 0,
            "enc_host_uin": "",
        }),
    )?;
    if let Some(info) = data.get("dirinfo") {
        api::remember_cover(
            &items::playlist_id(id),
            text(info, &["picurl", "picUrl", "cover"]).as_deref(),
        );
    }
    let songs = model::tracks(data.get("songlist").unwrap_or(&Value::Null));
    let total = number(&data, &["total_song_num", "songnum", "total"])
        .or_else(|| {
            data.get("dirinfo")
                .and_then(|d| number(d, &["songnum", "song_num"]))
        })
        .map(|t| t as usize);
    Ok(paging::served(items::tracks(&songs), offset, limit, total))
}

/// A page of an album's songs.
fn album_page(mid: &str, page: &PageRequest) -> Result<ItemPage> {
    let offset = paging::offset(page)?;
    let limit = paging::limit(page);
    let data = api::call(
        "music.musichallAlbum.AlbumSongList",
        "GetAlbumSongList",
        json!({ "albumMid": mid, "albumID": 0, "begin": offset, "num": limit, "order": 2 }),
    )?;
    let songs = model::tracks(data.get("songList").unwrap_or(&Value::Null));
    let total = number(&data, &["totalNum", "total"]).map(|t| t as usize);
    Ok(paging::served(items::tracks(&songs), offset, limit, total))
}

/// A page of a singer's albums.
fn artist_albums(mid: &str, page: &PageRequest) -> Result<ItemPage> {
    let offset = paging::offset(page)?;
    let limit = paging::limit(page);
    let data = api::call(
        "music.musichallAlbum.AlbumListServer",
        "GetAlbumList",
        json!({ "singerMid": mid, "order": 0, "begin": offset, "num": limit, "songNumTag": 0, "singerID": 0 }),
    )?;
    let albums: Vec<Item> = model::list(&data, "albumList")
        .iter()
        .filter_map(items::album)
        .collect();
    let total = number(&data, &["total", "totalNum"]).map(|t| t as usize);
    Ok(paging::served(albums, offset, limit, total))
}

/// One song's details.
pub fn song(mid: &str) -> Result<Track> {
    let data = api::call(
        "music.pf_song_detail_svr",
        "get_song_detail_yqq",
        json!({ "song_type": 0, "song_mid": mid }),
    )?;
    data.get("track_info")
        .and_then(model::track)
        .ok_or(PluginError::NotFound)
}

/// A playlist's cover, from its details.
pub fn playlist_cover(id: &str) -> Result<Option<String>> {
    let data = api::call(
        "music.srfDissInfo.aiDissInfo",
        "uniform_get_Dissinfo",
        json!({
            "disstid": id.parse::<u64>().unwrap_or(0), "userinfo": 1, "tag": 0, "orderlist": 0,
            "song_begin": 0, "song_num": 1, "onlysonglist": 0, "enc_host_uin": "",
        }),
    )?;
    Ok(data
        .get("dirinfo")
        .and_then(|d| text(d, &["picurl", "picUrl", "cover"])))
}

/// Finds QQ Music's own song for an item from another source.
pub fn find_song(item: &Item) -> Result<Option<Track>> {
    let title = item.title.trim();
    if title.is_empty() {
        return Ok(None);
    }
    let query = match item.artist.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
        Some(artist) => format!("{title} {}", matching::primary_artist(artist)),
        None => title.to_string(),
    };
    let (entries, _) = search_page(&query, TRACKS, 1, 10)?;
    let songs = model::tracks(&Value::Array(entries));
    let artists: Vec<Option<String>> = songs
        .iter()
        .map(|s| jf_community_common::join_artists(s.singer.iter().map(|x| x.name.as_str())))
        .collect();
    let paired: Vec<(&Track, Option<&str>)> =
        songs.iter().zip(artists.iter().map(|a| a.as_deref())).collect();
    Ok(matching::best(item, &paired, |(song, artist)| Candidate {
        title: song.title(),
        artist: *artist,
        album: Some(song.album.name.as_str()).filter(|a| !a.is_empty()),
        duration_ms: u32::try_from(song.interval * 1000).ok().filter(|ms| *ms > 0),
    })
    .map(|(song, _)| (*song).clone()))
}

/// Finds QQ Music's own album for an album item from another source; its mid.
pub fn find_album(item: &Item) -> Result<Option<String>> {
    let title = item.title.trim();
    if title.is_empty() {
        return Ok(None);
    }
    let (entries, _) = search_page(title, 1, 1, 10)?;
    let found: Vec<Item> = entries.iter().filter_map(items::album).collect();
    let mut wanted = item.clone();
    wanted.duration_ms = None;
    Ok(matching::best(&wanted, &found, |album| Candidate {
        title: &album.title,
        artist: album.artist.as_deref(),
        album: None,
        duration_ms: None,
    })
    .and_then(|album| album.id.strip_prefix("qqm:al:").map(str::to_string)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_cursors_round_trip() {
        let cursor = SearchCursor::parse(Some("2x5,-,0,3x40")).unwrap();
        assert_eq!(cursor.next, [Some((2, 5)), None, None, Some((3, 40))]);
        assert_eq!(cursor.done, [false, true, false, false]);
        assert_eq!(cursor.encode(), "2x5,-,0,3x40");
        assert_eq!(SearchCursor::parse(None).unwrap(), SearchCursor::default());
        for bad in [
            "1,2,3",
            "a,b,c,d",
            "0x5,0,0,0",
            "1x0,0,0,0",
            "1x500,0,0,0",
            "1-2,0,0,0",
        ] {
            assert!(SearchCursor::parse(Some(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn shares_a_page_between_kinds() {
        assert_eq!(allocation([true; 4], 50), [5, 5, 5, 35]);
        assert_eq!(allocation([false, false, false, true], 20), [0, 0, 0, 20]);
        assert_eq!(allocation([true, true, false, false], 7), [4, 3, 0, 0]);
        assert_eq!(allocation([false; 4], 9), [0; 4]);
    }
}
