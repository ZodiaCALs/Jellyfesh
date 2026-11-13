//! Item ids and turning Subsonic objects into items.
//!
//! Ids are the server's own id behind a kind prefix, `tr:` (track), `al:`
//! (album), `ar:` (artist) or `pl:` (playlist), so they stay stable and the
//! plugin can tell its own items from items of other sources. Sections of
//! the browse tree are `f:` folders.
//!
//! Items carry no cover URL: a Subsonic cover URL has to contain the sign-in,
//! and items may be stored by the library. The host asks `artwork` instead,
//! which knows each listed item's cover id without another request.

use jf_plugin_sdk::prelude::*;

use crate::api;
use crate::model::{Album, Artist, Playlist, Song};

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
    /// Parses an id the plugin made; none for anything else (another
    /// source's item, or garbage).
    pub fn parse(id: &'a str) -> Option<Id<'a>> {
        let (prefix, rest) = id.split_once(':')?;
        if rest.is_empty() {
            return None;
        }
        Some(match prefix {
            "tr" => Id::Track(rest),
            "al" => Id::Album(rest),
            "ar" => Id::Artist(rest),
            "pl" => Id::Playlist(rest),
            "f" => Id::Folder(rest),
            _ => return None,
        })
    }
}

/// `tr:<id>`.
pub fn track_id(id: &str) -> String {
    format!("tr:{id}")
}

/// `al:<id>`.
pub fn album_id(id: &str) -> String {
    format!("al:{id}")
}

/// `ar:<id>`.
pub fn artist_id(id: &str) -> String {
    format!("ar:{id}")
}

/// `pl:<id>`.
pub fn playlist_id(id: &str) -> String {
    format!("pl:{id}")
}

/// A section of the browse tree.
pub fn folder(id: &str, title: &str) -> Item {
    Item::new(ItemKind::Folder, format!("f:{id}"), title)
}

fn ms(seconds: Option<u32>) -> Option<u32> {
    seconds.map(|s| s.saturating_mul(1000))
}

fn explicit(status: &Option<String>) -> bool {
    status
        .as_deref()
        .is_some_and(|s| s.eq_ignore_ascii_case("explicit"))
}

/// Why a song cannot be played here, if it cannot.
pub fn unplayable(song: &Song, can_stream: bool) -> Option<Unplayable> {
    if song.is_video == Some(true) {
        return Some(Unplayable {
            reason: UnplayableReason::Unavailable,
            message: Some("Videos can't be played in Jellyfesh".into()),
        });
    }
    if !can_stream {
        return Some(Unplayable {
            reason: UnplayableReason::Other,
            message: Some("Your account on this server is not allowed to stream music".into()),
        });
    }
    None
}

/// A song as a track item. `can_stream` is the account's permission.
pub fn track(song: &Song, can_stream: bool) -> Option<Item> {
    if song.id.is_empty() || song.is_dir == Some(true) {
        return None;
    }
    let id = track_id(&song.id);
    api::remember_cover(&id, &song.cover_art);
    let mut item = Item::new(ItemKind::Track, id, song.title.clone());
    item.artist = song.artist.clone().or_else(|| song.display_artist.clone());
    item.album = song.album.clone();
    item.album_id = song.album_id.as_deref().map(album_id);
    item.artist_id = song.artist_id.as_deref().map(artist_id);
    item.duration_ms = ms(song.duration);
    item.track_number = song.track.filter(|n| *n > 0);
    item.disc_number = song.disc_number.filter(|n| *n > 0);
    item.year = song.year.filter(|y| *y > 0);
    item.isrc = song.isrc.first().cloned();
    item.explicit = explicit(&song.explicit_status);
    if let Some(reason) = unplayable(song, can_stream) {
        item.playable = false;
        item.unplayable = Some(reason);
    }
    Some(item)
}

/// The title of an album, whichever field the server uses.
pub fn album_title(album: &Album) -> String {
    album
        .name
        .clone()
        .or_else(|| album.title.clone())
        .or_else(|| album.album.clone())
        .unwrap_or_default()
}

/// An album item.
pub fn album(album: &Album) -> Option<Item> {
    if album.id.is_empty() {
        return None;
    }
    let id = album_id(&album.id);
    api::remember_cover(&id, &album.cover_art);
    let mut item = Item::new(ItemKind::Album, id, album_title(album));
    item.artist = album.artist.clone().or_else(|| album.display_artist.clone());
    item.artist_id = album.artist_id.as_deref().map(artist_id);
    item.duration_ms = ms(album.duration);
    item.year = album.year.filter(|y| *y > 0);
    item.child_count = album.song_count;
    item.explicit = explicit(&album.explicit_status);
    Some(item)
}

/// An artist item.
pub fn artist(artist: &Artist) -> Option<Item> {
    if artist.id.is_empty() {
        return None;
    }
    let id = artist_id(&artist.id);
    api::remember_cover(&id, &artist.cover_art);
    let mut item = Item::new(ItemKind::Artist, id, artist.name.clone());
    item.child_count = artist.album_count;
    Some(item)
}

/// A playlist item.
pub fn playlist(playlist: &Playlist) -> Option<Item> {
    if playlist.id.is_empty() {
        return None;
    }
    let id = playlist_id(&playlist.id);
    api::remember_cover(&id, &playlist.cover_art);
    let mut item = Item::new(ItemKind::Playlist, id, playlist.name.clone());
    item.duration_ms = ms(playlist.duration);
    item.child_count = playlist.song_count;
    Some(item)
}

/// Tracks for songs, with the account's permission looked up once.
pub fn tracks(songs: &[Song]) -> Vec<Item> {
    if songs.is_empty() {
        return Vec::new();
    }
    let can_stream = api::can_stream();
    songs.iter().filter_map(|s| track(s, can_stream)).collect()
}

/// A slice of a whole list, with the cursor of the next slice.
pub fn slice(items: Vec<Item>, page: &PageRequest) -> Result<ItemPage> {
    let start = offset(page)?;
    let total = items.len();
    let limit = page.limit.max(1) as usize;
    let end = (start + limit).min(total);
    let chunk = items
        .into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect();
    Ok(ItemPage {
        items: chunk,
        next: (end < total).then(|| end.to_string()),
        total: u32::try_from(total).ok(),
    })
}

/// The offset a cursor stands for (none: 0).
pub fn offset(page: &PageRequest) -> Result<usize> {
    match page.cursor.as_deref() {
        None | Some("") => Ok(0),
        Some(cursor) => cursor
            .parse()
            .map_err(|_| PluginError::invalid_input(format!("bad cursor {cursor:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ids() {
        assert_eq!(Id::parse("tr:al-12"), Some(Id::Track("al-12")));
        assert_eq!(Id::parse("al:a:b"), Some(Id::Album("a:b")));
        assert_eq!(Id::parse("f:albums:newest"), Some(Id::Folder("albums:newest")));
        assert_eq!(Id::parse("tr:"), None);
        assert_eq!(Id::parse("12345"), None);
        assert_eq!(Id::parse("song:1"), None);
    }

    #[test]
    fn marks_unplayable_songs() {
        let video = Song {
            is_video: Some(true),
            ..Song::default()
        };
        assert_eq!(
            unplayable(&video, true).unwrap().reason,
            UnplayableReason::Unavailable
        );
        let song = Song::default();
        assert!(unplayable(&song, true).is_none());
        assert_eq!(unplayable(&song, false).unwrap().reason, UnplayableReason::Other);
    }

    #[test]
    fn slices_pages() {
        let items: Vec<Item> = (0..5)
            .map(|i| Item::new(ItemKind::Track, i.to_string(), "t"))
            .collect();
        let first = slice(
            items.clone(),
            &PageRequest {
                cursor: None,
                limit: 2,
            },
        )
        .unwrap();
        assert_eq!(first.items.len(), 2);
        assert_eq!((first.next.as_deref(), first.total), (Some("2"), Some(5)));
        let last = slice(
            items.clone(),
            &PageRequest {
                cursor: Some("4".into()),
                limit: 2,
            },
        )
        .unwrap();
        assert_eq!((last.items.len(), last.next), (1, None));
        let past = slice(
            items.clone(),
            &PageRequest {
                cursor: Some("9".into()),
                limit: 2,
            },
        )
        .unwrap();
        assert!(past.items.is_empty() && past.next.is_none());
        assert!(
            slice(
                items,
                &PageRequest {
                    cursor: Some("x".into()),
                    limit: 2
                }
            )
            .is_err()
        );
    }
}
