//! Item ids and turning QQ Music objects into items.
//!
//! Ids are QQ Music's own `mid`s (songs, albums, singers) or numeric
//! playlist ids behind `qqm:` and a kind: `qqm:tr:`, `qqm:al:`, `qqm:ar:`,
//! `qqm:pl:`; sections of the library are `qqm:f:` folders. The `qqm:`
//! prefix keeps them apart from other sources' ids.
//!
//! Song listings do not say what this account may play (only what a song
//! costs in general), so a track is marked unplayable when it has no audio
//! at all, or when the stream service already refused it to this account;
//! otherwise the stream request decides, and says why when it refuses.

use jf_community_common::{join_artists, non_empty, year_from_text};
use jf_plugin_sdk::prelude::*;
use serde_json::Value;

use crate::api;
use crate::model::{Track, number, text};

/// Default picture size on items.
const ITEM_ART: u32 = 300;

/// One of the plugin's ids, parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Id<'a> {
    Track(&'a str),
    Album(&'a str),
    Artist(&'a str),
    Playlist(&'a str),
    Folder(&'a str),
}

impl<'a> Id<'a> {
    /// Parses an id the plugin made; `None` for anything else.
    pub fn parse(id: &'a str) -> Option<Id<'a>> {
        let rest = id.strip_prefix("qqm:")?;
        let (kind, value) = rest.split_once(':')?;
        if value.is_empty() || value.len() > 64 {
            return None;
        }
        if kind == "f" {
            return Some(Id::Folder(value));
        }
        if !value.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return None;
        }
        Some(match kind {
            "tr" => Id::Track(value),
            "al" => Id::Album(value),
            "ar" => Id::Artist(value),
            "pl" if value.bytes().all(|b| b.is_ascii_digit()) => Id::Playlist(value),
            _ => return None,
        })
    }
}

/// `qqm:tr:<mid>`.
pub fn track_id(mid: &str) -> String {
    format!("qqm:tr:{mid}")
}

/// `qqm:al:<mid>`.
pub fn album_id(mid: &str) -> String {
    format!("qqm:al:{mid}")
}

/// `qqm:ar:<mid>`.
pub fn artist_id(mid: &str) -> String {
    format!("qqm:ar:{mid}")
}

/// `qqm:pl:<id>`.
pub fn playlist_id(id: &str) -> String {
    format!("qqm:pl:{id}")
}

/// A section of the library.
pub fn folder(id: &str, title: &str) -> Item {
    Item::new(ItemKind::Folder, format!("qqm:f:{id}"), title)
}

/// The message for a song without audio.
pub const NO_AUDIO: &str = "QQ Music has no audio for this track";

/// A song as a track item.
pub fn track(song: &Track) -> Option<Item> {
    if song.mid.is_empty() {
        return None;
    }
    let title = match song.title() {
        "" => format!("Track {}", song.mid),
        title => jf_community_common::entities::decode(title),
    };
    let mut item = Item::new(ItemKind::Track, track_id(&song.mid), title);
    item.artist = join_artists(song.singer.iter().map(|s| s.name.as_str()))
        .map(|a| jf_community_common::entities::decode(&a));
    item.artist_id = song
        .singer
        .first()
        .filter(|s| !s.mid.is_empty())
        .map(|s| artist_id(&s.mid));
    item.album = non_empty(Some(&song.album.name)).map(|a| jf_community_common::entities::decode(&a));
    if !song.album.mid.is_empty() {
        item.album_id = Some(album_id(&song.album.mid));
        item.artwork = api::album_picture(&song.album.mid, ITEM_ART);
    }
    item.duration_ms = u32::try_from(song.interval.saturating_mul(1000))
        .ok()
        .filter(|ms| *ms > 0);
    item.track_number = u32::try_from(song.index_album).ok().filter(|n| *n > 0);
    item.year = year_from_text(&song.time_public);
    item.isrc = non_empty(Some(&song.isrc));
    let refusal = if !song.file.any() && song.file.media_mid.is_empty() {
        Some(Unplayable {
            reason: UnplayableReason::Unavailable,
            message: Some(NO_AUDIO.into()),
        })
    } else {
        api::refusal_of(&song.mid)
    };
    if let Some(why) = refusal {
        item.playable = false;
        item.unplayable = Some(why);
    }
    Some(item)
}

/// Songs as tracks.
pub fn tracks(songs: &[Track]) -> Vec<Item> {
    songs.iter().filter_map(track).collect()
}

/// An album from a search result or an artist's album list.
pub fn album(entry: &Value) -> Option<Item> {
    let mid = text(entry, &["albumMID", "albumMid", "albummid", "album_mid", "mid"])?;
    if !mid.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    let title = text(entry, &["albumName", "albumname", "album_name", "name", "title"])
        .map(|t| jf_community_common::entities::decode(&t))
        .unwrap_or_else(|| format!("Album {mid}"));
    let mut item = Item::new(ItemKind::Album, album_id(&mid), title);
    let singers = entry
        .get("singer_list")
        .or_else(|| entry.get("singers"))
        .or_else(|| entry.get("singer"))
        .and_then(Value::as_array);
    item.artist = singers
        .and_then(|list| join_artists(list.iter().filter_map(|s| s.get("name").and_then(Value::as_str))))
        .or_else(|| text(entry, &["singerName", "singername", "singer_name"]))
        .map(|a| jf_community_common::entities::decode(&a));
    item.artist_id = singers
        .and_then(|list| list.first())
        .and_then(|s| text(s, &["mid", "singerMid", "singerMID"]))
        .map(|mid| artist_id(&mid));
    item.year = text(
        entry,
        &[
            "publicTime",
            "publishDate",
            "publish_date",
            "pub_time",
            "publictime",
        ],
    )
    .and_then(|date| year_from_text(&date));
    item.child_count = number(
        entry,
        &["song_count", "songNum", "song_num", "totalNum", "total_song_num"],
    )
    .and_then(|n| u32::try_from(n).ok())
    .filter(|n| *n > 0);
    item.artwork = api::album_picture(&mid, ITEM_ART);
    Some(item)
}

/// A singer from a search result.
pub fn artist(entry: &Value) -> Option<Item> {
    let mid = text(
        entry,
        &["singerMID", "singerMid", "singermid", "singer_mid", "mid"],
    )?;
    if !mid.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    let title = text(
        entry,
        &["singerName", "singername", "singer_name", "name", "title"],
    )
    .map(|t| jf_community_common::entities::decode(&t))
    .unwrap_or_else(|| format!("Singer {mid}"));
    let mut item = Item::new(ItemKind::Artist, artist_id(&mid), title);
    item.child_count = number(entry, &["albumNum", "album_num", "albumnum"])
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0);
    item.artwork = api::singer_picture(&mid, ITEM_ART);
    Some(item)
}

/// A playlist from a search result.
pub fn playlist(entry: &Value) -> Option<Item> {
    let id = text(entry, &["dissid", "disstid", "tid", "id"])?;
    if !id.bytes().all(|b| b.is_ascii_digit()) || id == "0" {
        return None;
    }
    let title = text(entry, &["dissname", "diss_name", "title", "name"])
        .map(|t| jf_community_common::entities::decode(&t))
        .unwrap_or_else(|| format!("Playlist {id}"));
    let mut item = Item::new(ItemKind::Playlist, playlist_id(&id), title);
    item.artist = entry
        .get("creator")
        .and_then(|c| text(c, &["name", "nick", "nickname"]))
        .map(|a| jf_community_common::entities::decode(&a));
    item.child_count =
        number(entry, &["song_count", "songnum", "songNum", "song_num"]).and_then(|n| u32::try_from(n).ok());
    let cover = text(entry, &["imgurl", "logo", "picurl", "picUrl", "cover"]);
    api::remember_cover(&item.id, cover.as_deref());
    item.artwork = cover.as_deref().and_then(api::picture);
    Some(item)
}

/// One of the account's own playlists.
pub fn own_playlist(list: &api::OwnPlaylist) -> Item {
    let mut item = Item::new(ItemKind::Playlist, playlist_id(&list.id), list.title.clone());
    item.child_count = list.count;
    api::remember_cover(&item.id, list.cover.as_deref());
    item.artwork = list.cover.as_deref().and_then(api::picture);
    item
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_ids() {
        assert_eq!(
            Id::parse("qqm:tr:0000MockSong01"),
            Some(Id::Track("0000MockSong01"))
        );
        assert_eq!(Id::parse("qqm:pl:8000002"), Some(Id::Playlist("8000002")));
        assert_eq!(Id::parse("qqm:f:liked"), Some(Id::Folder("liked")));
        for other in [
            "tr:x",
            "qqm:tr:",
            "qqm:tr:a/b",
            "qqm:pl:abc",
            "qqm:zz:1",
            "qqm:",
            "x",
        ] {
            assert_eq!(Id::parse(other), None, "{other}");
        }
    }

    #[test]
    fn maps_a_song() {
        let song = crate::model::track(&json!({
            "mid": "0000MockSong01", "name": "Mock &amp; Song",
            "singer": [{ "mid": "0000MockSinger", "name": "Mock Singer" }, { "mid": "g", "name": "Guest" }],
            "album": { "mid": "0000MockAlbum1", "name": "Mock Album" },
            "interval": 200, "index_album": 2, "time_public": "2020-05-01", "isrc": "CNA001234567",
            "file": { "media_mid": "m", "size_128mp3": 1 }
        }))
        .unwrap();
        let item = track(&song).unwrap();
        assert_eq!(item.title, "Mock & Song");
        assert_eq!(item.artist.as_deref(), Some("Mock Singer, Guest"));
        assert_eq!(item.artist_id.as_deref(), Some("qqm:ar:0000MockSinger"));
        assert_eq!(item.album_id.as_deref(), Some("qqm:al:0000MockAlbum1"));
        assert_eq!(
            (item.duration_ms, item.track_number, item.year),
            (Some(200_000), Some(2), Some(2020))
        );
        assert!(item.playable);
        assert!(
            item.artwork
                .unwrap()
                .url
                .contains("T002R300x300M0000000MockAlbum1.jpg")
        );
        let silent =
            crate::model::track(&json!({ "mid": "0000Missing001", "name": "x", "file": {} })).unwrap();
        let silent = track(&silent).unwrap();
        assert!(!silent.playable);
        assert_eq!(silent.unplayable.unwrap().reason, UnplayableReason::Unavailable);
    }

    #[test]
    fn maps_search_entries() {
        let listed = album(&json!({
            "albumMID": "0000MockAlbum1", "albumName": "Mock Album", "publicTime": "2019-02-03",
            "singer_list": [{ "mid": "0000MockSinger", "name": "Mock Singer" }], "song_count": 3
        }))
        .unwrap();
        assert_eq!(
            (listed.id.as_str(), listed.year, listed.child_count),
            ("qqm:al:0000MockAlbum1", Some(2019), Some(3))
        );
        assert_eq!(listed.artist_id.as_deref(), Some("qqm:ar:0000MockSinger"));
        let singer = artist(&json!({ "singerMID": "0000MockSinger", "singerName": "Mock Singer" })).unwrap();
        assert_eq!(singer.title, "Mock Singer");
        let list = playlist(
            &json!({ "dissid": "8000003", "dissname": "Mock Mix", "imgurl": "http://qpic.y.qq.com/c/1" }),
        )
        .unwrap();
        assert_eq!(list.id, "qqm:pl:8000003");
        assert_eq!(list.artwork.unwrap().url, "https://qpic.y.qq.com/c/1");
        assert!(playlist(&json!({ "dissid": "abc" })).is_none());
        assert!(album(&json!({ "albumName": "no mid" })).is_none());
    }
}
