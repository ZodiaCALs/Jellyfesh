//! Item ids and turning Jellyfin items into Jellyfesh items.
//!
//! Ids are the server's item id behind a kind prefix: `tr:` (track), `al:`
//! (album), `ar:` (artist), `pl:` (playlist), and `f:` for sections of the
//! browse tree (`f:lib:<library id>:albums`, `f:favourites:songs`, …).
//!
//! Items carry their cover as an image URL: Jellyfin serves item images
//! without a sign-in (its web app loads them in plain `<img>` tags), so the
//! URL holds no credential and may be stored with the item.

use jf_plugin_sdk::prelude::*;

use crate::api;
use crate::model::BaseItem;

/// Default artwork size in listings, in pixels.
pub const LIST_ART: u32 = 300;
/// 100-nanosecond ticks per millisecond.
pub const TICKS_PER_MS: i64 = 10_000;

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

    /// The server's id (none for folders).
    pub fn server_id(self) -> Option<&'a str> {
        match self {
            Id::Track(id) | Id::Album(id) | Id::Artist(id) | Id::Playlist(id) => Some(id),
            Id::Folder(_) => None,
        }
    }
}

/// A section of the browse tree.
pub fn folder(id: &str, title: &str) -> Item {
    Item::new(ItemKind::Folder, format!("f:{id}"), title)
}

fn ms(ticks: Option<i64>) -> Option<u32> {
    ticks
        .filter(|t| *t > 0)
        .map(|t| u32::try_from(t / TICKS_PER_MS).unwrap_or(u32::MAX))
}

fn positive(n: Option<i64>) -> Option<u32> {
    n.filter(|n| *n > 0).and_then(|n| u32::try_from(n).ok())
}

/// The image that shows `item`: its own Primary image, or its album's.
pub fn image(item: &BaseItem) -> Option<(String, String)> {
    if let Some(tag) = item.image_tags.get("Primary") {
        return Some((item.id.clone(), tag.clone()));
    }
    match (&item.album_id, &item.album_primary_image_tag) {
        (Some(album), Some(tag)) if !album.is_empty() => Some((album.clone(), tag.clone())),
        _ => None,
    }
}

/// The URL of an item's Primary image about `size` pixels square.
pub fn image_url(image_item: &str, tag: Option<&str>, size: u32) -> Result<String> {
    let size = size.clamp(16, 3000).to_string();
    let mut params = vec![
        ("fillWidth", size.as_str()),
        ("fillHeight", size.as_str()),
        ("quality", "90"),
    ];
    if let Some(tag) = tag {
        params.push(("tag", tag));
    }
    Ok(format!(
        "{}/Items/{}/Images/Primary?{}",
        api::base()?,
        http::encode(image_item),
        http::form_encode(&params)
    ))
}

fn artwork(item: &BaseItem, id: &str) -> Option<ImageRef> {
    let (image_item, tag) = image(item)?;
    api::remember_image(id, &(image_item.clone(), tag.clone()));
    Some(ImageRef::new(image_url(&image_item, Some(&tag), LIST_ART).ok()?))
}

/// Why a track cannot be played, if it cannot.
pub fn unplayable(item: &BaseItem) -> Option<Unplayable> {
    if item.location_type.as_deref() == Some("Virtual") {
        return Some(Unplayable {
            reason: UnplayableReason::Unavailable,
            message: Some("The file of this track is missing on the server".into()),
        });
    }
    if item.play_access.as_deref() == Some("None") {
        return Some(Unplayable {
            reason: UnplayableReason::Other,
            message: Some("Your account on this server is not allowed to play media".into()),
        });
    }
    None
}

fn names(list: &[crate::model::NameId]) -> Option<String> {
    let names: Vec<&str> = list
        .iter()
        .filter_map(|n| n.name.as_deref())
        .filter(|n| !n.is_empty())
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

/// Any Jellyfin item as an item, by its type; none for types the plugin
/// does not show.
pub fn item(item: &BaseItem) -> Option<Item> {
    match item.kind.as_deref()? {
        "Audio" => track(item),
        "MusicAlbum" => album(item),
        "MusicArtist" => artist(item),
        "Playlist" => playlist(item),
        _ => None,
    }
}

/// Items for a list, skipping types the plugin does not show.
pub fn items(list: &[BaseItem]) -> Vec<Item> {
    list.iter().filter_map(item).collect()
}

/// A track.
pub fn track(source: &BaseItem) -> Option<Item> {
    if source.id.is_empty() {
        return None;
    }
    let id = format!("tr:{}", source.id);
    let mut out = Item::new(
        ItemKind::Track,
        id.clone(),
        source.name.clone().unwrap_or_default(),
    );
    out.artist = if source.artists.is_empty() {
        source.album_artist.clone().filter(|a| !a.is_empty())
    } else {
        Some(source.artists.join(", "))
    };
    out.album = source.album.clone().filter(|a| !a.is_empty());
    out.album_id = source
        .album_id
        .as_deref()
        .filter(|a| !a.is_empty())
        .map(|a| format!("al:{a}"));
    out.artist_id = source
        .artist_items
        .first()
        .or_else(|| source.album_artists.first())
        .filter(|a| !a.id.is_empty())
        .map(|a| format!("ar:{}", a.id));
    out.duration_ms = ms(source.run_time_ticks);
    out.track_number = positive(source.index_number);
    out.disc_number = positive(source.parent_index_number);
    out.year = positive(source.production_year);
    out.artwork = artwork(source, &id);
    if let Some(reason) = unplayable(source) {
        out.playable = false;
        out.unplayable = Some(reason);
    }
    Some(out)
}

/// An album.
pub fn album(source: &BaseItem) -> Option<Item> {
    if source.id.is_empty() {
        return None;
    }
    let id = format!("al:{}", source.id);
    let mut out = Item::new(
        ItemKind::Album,
        id.clone(),
        source.name.clone().unwrap_or_default(),
    );
    out.artist = source
        .album_artist
        .clone()
        .filter(|a| !a.is_empty())
        .or_else(|| names(&source.album_artists));
    out.artist_id = source
        .album_artists
        .first()
        .filter(|a| !a.id.is_empty())
        .map(|a| format!("ar:{}", a.id));
    out.duration_ms = ms(source.run_time_ticks);
    out.year = positive(source.production_year);
    out.child_count = positive(source.child_count);
    out.artwork = artwork(source, &id);
    Some(out)
}

/// An artist.
pub fn artist(source: &BaseItem) -> Option<Item> {
    if source.id.is_empty() {
        return None;
    }
    let id = format!("ar:{}", source.id);
    let mut out = Item::new(
        ItemKind::Artist,
        id.clone(),
        source.name.clone().unwrap_or_default(),
    );
    out.artwork = artwork(source, &id);
    Some(out)
}

/// A playlist.
pub fn playlist(source: &BaseItem) -> Option<Item> {
    if source.id.is_empty() {
        return None;
    }
    let id = format!("pl:{}", source.id);
    let mut out = Item::new(
        ItemKind::Playlist,
        id.clone(),
        source.name.clone().unwrap_or_default(),
    );
    out.duration_ms = ms(source.run_time_ticks);
    out.child_count = positive(source.child_count);
    out.artwork = artwork(source, &id);
    Some(out)
}

/// The start index a cursor stands for (none: 0).
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
    use crate::model::NameId;

    #[test]
    fn parses_ids() {
        assert_eq!(Id::parse("tr:0a1b"), Some(Id::Track("0a1b")));
        assert_eq!(Id::parse("f:lib:9c:albums"), Some(Id::Folder("lib:9c:albums")));
        assert_eq!(Id::parse("al:"), None);
        assert_eq!(Id::parse("0a1b"), None);
        assert_eq!(Id::parse("tr:x").and_then(Id::server_id), Some("x"));
        assert_eq!(Id::parse("f:x").and_then(Id::server_id), None);
    }

    #[test]
    fn marks_unplayable_tracks() {
        let missing = BaseItem {
            location_type: Some("Virtual".into()),
            ..BaseItem::default()
        };
        assert_eq!(
            unplayable(&missing).unwrap().reason,
            UnplayableReason::Unavailable
        );
        let refused = BaseItem {
            play_access: Some("None".into()),
            ..BaseItem::default()
        };
        assert_eq!(unplayable(&refused).unwrap().reason, UnplayableReason::Other);
        assert!(unplayable(&BaseItem::default()).is_none());
    }

    #[test]
    fn picks_the_image() {
        let mut item = BaseItem {
            id: "t1".into(),
            album_id: Some("a1".into()),
            album_primary_image_tag: Some("at".into()),
            ..BaseItem::default()
        };
        assert_eq!(image(&item), Some(("a1".into(), "at".into())));
        item.image_tags.insert("Primary".into(), "own".into());
        assert_eq!(image(&item), Some(("t1".into(), "own".into())));
        assert_eq!(image(&BaseItem::default()), None);
        let joined = names(&[
            NameId {
                name: Some("A".into()),
                id: "1".into(),
            },
            NameId {
                name: Some("B".into()),
                id: "2".into(),
            },
        ]);
        assert_eq!(joined.as_deref(), Some("A, B"));
    }
}
