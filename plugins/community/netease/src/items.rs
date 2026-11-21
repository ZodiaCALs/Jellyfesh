//! Item ids and turning NetEase objects into items.
//!
//! Ids are NetEase's numeric ids behind `ncm:` and a kind: `ncm:tr:`
//! (track), `ncm:al:` (album), `ncm:ar:` (artist), `ncm:pl:` (playlist);
//! sections of the library are `ncm:f:` folders. The `ncm:` prefix keeps
//! them apart from other sources' ids when Jellyfesh asks for lyrics or
//! artwork of an item from elsewhere.
//!
//! Whether a track may be played comes from the account's `privilege` for
//! it, which NetEase sends with every listing: no rights at all
//! (`unavailable`), a subscription or a purchase needed (`paywall`). The
//! plugin never offers a preview in place of the song.

use jf_community_common::{join_artists, non_empty, year_from_epoch_ms};
use jf_plugin_sdk::prelude::*;

use crate::api;
use crate::model::{Album, Artist, Playlist, Privilege, Song, to_u32};

/// Default picture size on items.
const ITEM_ART: u32 = 300;
/// NetEase dates releases in China time.
const UTC_OFFSET_HOURS: i64 = 8;

/// One of the plugin's ids, parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Id<'a> {
    Track(u64),
    Album(u64),
    Artist(u64),
    Playlist(u64),
    Folder(&'a str),
}

impl<'a> Id<'a> {
    /// Parses an id the plugin made; `None` for anything else (another
    /// source's item, or garbage).
    pub fn parse(id: &'a str) -> Option<Id<'a>> {
        let rest = id.strip_prefix("ncm:")?;
        let (kind, value) = rest.split_once(':')?;
        if value.is_empty() {
            return None;
        }
        if kind == "f" {
            return Some(Id::Folder(value));
        }
        let number: u64 = value.parse().ok().filter(|n| *n > 0)?;
        Some(match kind {
            "tr" => Id::Track(number),
            "al" => Id::Album(number),
            "ar" => Id::Artist(number),
            "pl" => Id::Playlist(number),
            _ => return None,
        })
    }
}

/// `ncm:tr:<id>`.
pub fn track_id(id: u64) -> String {
    format!("ncm:tr:{id}")
}

/// `ncm:al:<id>`.
pub fn album_id(id: u64) -> String {
    format!("ncm:al:{id}")
}

/// `ncm:ar:<id>`.
pub fn artist_id(id: u64) -> String {
    format!("ncm:ar:{id}")
}

/// `ncm:pl:<id>`.
pub fn playlist_id(id: u64) -> String {
    format!("ncm:pl:{id}")
}

/// A section of the library.
pub fn folder(id: &str, title: &str) -> Item {
    Item::new(ItemKind::Folder, format!("ncm:f:{id}"), title)
}

fn art(item: &mut Item, pic_url: Option<&str>) {
    api::remember_cover(&item.id, pic_url);
    item.artwork = pic_url.and_then(|url| api::image(url, ITEM_ART));
}

/// Why the account cannot play a song, if it cannot, from its privilege.
pub fn unplayable(song: &Song, privilege: Option<&Privilege>) -> Option<Unplayable> {
    if song.no_copyright.as_ref().is_some_and(|v| !v.is_null()) {
        return Some(Unplayable {
            reason: UnplayableReason::Unavailable,
            message: Some("NetEase Cloud Music has no rights to this track".into()),
        });
    }
    let privilege = privilege?;
    if privilege.st.is_some_and(|st| st < 0) {
        return Some(Unplayable {
            reason: UnplayableReason::Unavailable,
            message: Some("Not available on NetEase Cloud Music".into()),
        });
    }
    if privilege.pl == Some(0) {
        let fee = privilege.fee.or(song.fee);
        return Some(match fee {
            Some(1) => Unplayable {
                reason: UnplayableReason::Paywall,
                message: Some("Needs a NetEase Cloud Music VIP subscription".into()),
            },
            Some(4) => Unplayable {
                reason: UnplayableReason::Paywall,
                message: Some("Needs the album bought on NetEase Cloud Music".into()),
            },
            _ => Unplayable {
                reason: UnplayableReason::Unavailable,
                message: Some(
                    "NetEase Cloud Music does not let this account play this track (it may be limited to \
                     some regions)"
                        .into(),
                ),
            },
        });
    }
    None
}

/// A song as a track item. `privilege` overrides the song's own.
pub fn track(song: &Song, privilege: Option<&Privilege>) -> Option<Item> {
    if song.id == 0 {
        return None;
    }
    let title = non_empty(song.name.as_deref()).unwrap_or_else(|| format!("Track {}", song.id));
    let mut item = Item::new(ItemKind::Track, track_id(song.id), title);
    item.artist = join_artists(song.ar.iter().filter_map(|a| a.name.as_deref()));
    item.artist_id = song.ar.first().filter(|a| a.id > 0).map(|a| artist_id(a.id));
    if let Some(album) = &song.al {
        item.album = non_empty(album.name.as_deref());
        item.album_id = (album.id > 0).then(|| album_id(album.id));
        art(&mut item, album.pic_url.as_deref());
    }
    item.duration_ms = to_u32(song.dt).filter(|ms| *ms > 0);
    item.track_number = to_u32(song.no).filter(|n| *n > 0);
    item.disc_number = to_u32(song.cd).filter(|n| *n > 0);
    item.year = song
        .publish_time
        .and_then(|ms| year_from_epoch_ms(ms, UTC_OFFSET_HOURS));
    let privilege = privilege.or(song.privilege.as_ref());
    if let Some(reason) = unplayable(song, privilege) {
        item.playable = false;
        item.unplayable = Some(reason);
    }
    Some(item)
}

/// Songs as track items, with privileges from a separate list where the
/// answer has one.
pub fn tracks(songs: &[Song], privileges: &[Privilege]) -> Vec<Item> {
    songs
        .iter()
        .filter_map(|song| {
            let privilege = privileges.iter().find(|p| p.id == song.id);
            track(song, privilege)
        })
        .collect()
}

/// An album item.
pub fn album(album: &Album) -> Option<Item> {
    if album.id == 0 {
        return None;
    }
    let title = non_empty(album.name.as_deref()).unwrap_or_else(|| format!("Album {}", album.id));
    let mut item = Item::new(ItemKind::Album, album_id(album.id), title);
    let artists: Vec<&Artist> = if album.artists.is_empty() {
        album.artist.iter().collect()
    } else {
        album.artists.iter().collect()
    };
    item.artist = join_artists(artists.iter().filter_map(|a| a.name.as_deref()));
    item.artist_id = artists.first().filter(|a| a.id > 0).map(|a| artist_id(a.id));
    item.child_count = to_u32(album.size).filter(|n| *n > 0);
    item.year = album
        .publish_time
        .and_then(|ms| year_from_epoch_ms(ms, UTC_OFFSET_HOURS));
    art(&mut item, album.pic_url.as_deref());
    Some(item)
}

/// An artist item.
pub fn artist(artist: &Artist) -> Option<Item> {
    if artist.id == 0 {
        return None;
    }
    let title = non_empty(artist.name.as_deref()).unwrap_or_else(|| format!("Artist {}", artist.id));
    let mut item = Item::new(ItemKind::Artist, artist_id(artist.id), title);
    item.child_count = to_u32(artist.album_size).filter(|n| *n > 0);
    let pic = artist.pic_url.as_deref().or(artist.img1v1_url.as_deref());
    art(&mut item, pic);
    Some(item)
}

/// A playlist item.
pub fn playlist(playlist: &Playlist) -> Option<Item> {
    if playlist.id == 0 {
        return None;
    }
    let title = non_empty(playlist.name.as_deref()).unwrap_or_else(|| format!("Playlist {}", playlist.id));
    let mut item = Item::new(ItemKind::Playlist, playlist_id(playlist.id), title);
    item.artist = playlist
        .creator
        .as_ref()
        .and_then(|c| non_empty(c.nickname.as_deref()));
    item.child_count = to_u32(playlist.track_count);
    art(&mut item, playlist.cover());
    Some(item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn song(value: serde_json::Value) -> Song {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn parses_ids() {
        assert_eq!(Id::parse("ncm:tr:12"), Some(Id::Track(12)));
        assert_eq!(
            Id::parse("ncm:pl:9007199254740993"),
            Some(Id::Playlist(9_007_199_254_740_993))
        );
        assert_eq!(Id::parse("ncm:f:liked"), Some(Id::Folder("liked")));
        for other in [
            "tr:12", "ncm:tr:", "ncm:tr:x", "ncm:tr:0", "ncm:xx:1", "ncm:", "12",
        ] {
            assert_eq!(Id::parse(other), None, "{other}");
        }
    }

    #[test]
    fn privileges_decide_playability() {
        let free = song(json!({ "id": 1, "fee": 8 }));
        let ok = Privilege {
            id: 1,
            fee: Some(8),
            st: Some(0),
            pl: Some(128_000),
            maxbr: Some(999_000),
        };
        assert!(unplayable(&free, Some(&ok)).is_none());
        assert!(unplayable(&free, None).is_none(), "unknown: the stream decides");

        let vip = Privilege {
            pl: Some(0),
            fee: Some(1),
            ..ok.clone()
        };
        let reason = unplayable(&free, Some(&vip)).unwrap();
        assert_eq!(reason.reason, UnplayableReason::Paywall);
        assert!(reason.message.unwrap().contains("VIP"));

        let album = Privilege {
            pl: Some(0),
            fee: Some(4),
            ..ok.clone()
        };
        assert_eq!(
            unplayable(&free, Some(&album)).unwrap().reason,
            UnplayableReason::Paywall
        );

        let gone = Privilege {
            st: Some(-200),
            ..ok.clone()
        };
        assert_eq!(
            unplayable(&free, Some(&gone)).unwrap().reason,
            UnplayableReason::Unavailable
        );

        let blocked = Privilege {
            pl: Some(0),
            fee: Some(0),
            ..ok.clone()
        };
        assert_eq!(
            unplayable(&free, Some(&blocked)).unwrap().reason,
            UnplayableReason::Unavailable
        );

        let no_rights = song(json!({ "id": 1, "noCopyrightRcmd": { "type": 1 } }));
        assert_eq!(
            unplayable(&no_rights, Some(&ok)).unwrap().reason,
            UnplayableReason::Unavailable
        );
    }

    #[test]
    fn maps_a_song() {
        let s = song(json!({
            "id": 1001, "name": "Mock Song", "ar": [{ "id": 7, "name": "Mock Artist" }, { "id": 8, "name": "Guest" }],
            "al": { "id": 70, "name": "Mock Album", "picUrl": "http://p1.music.126.net/mock/70.jpg" },
            "dt": 201000, "no": 2, "cd": "1", "publishTime": 1577808000000_i64
        }));
        let item = track(&s, None).unwrap();
        assert_eq!(item.id, "ncm:tr:1001");
        assert_eq!(item.artist.as_deref(), Some("Mock Artist, Guest"));
        assert_eq!(item.artist_id.as_deref(), Some("ncm:ar:7"));
        assert_eq!(item.album_id.as_deref(), Some("ncm:al:70"));
        assert_eq!(
            (item.track_number, item.disc_number, item.year),
            (Some(2), Some(1), Some(2020))
        );
        assert_eq!(
            item.artwork.unwrap().url,
            "https://p1.music.126.net/mock/70.jpg?param=300y300"
        );
        assert!(item.playable);
        assert!(track(&song(json!({ "id": 0 })), None).is_none());
    }
}
