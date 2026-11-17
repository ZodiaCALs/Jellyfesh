//! Spotify's objects (Web API reference, 2026 schema) as Jellyfesh items.
//! Item ids are Spotify's bare ids, so `[links]` open them on
//! open.spotify.com; the library sections are folders (`playlists`,
//! `albums`, `tracks`).

use jf_plugin_sdk::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Image {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

/// The smallest picture at least 300 pixels wide, else the largest one.
pub fn artwork(images: &[Image]) -> Option<ImageRef> {
    let usable: Vec<&Image> = images.iter().filter(|i| i.url.starts_with("https://")).collect();
    let chosen = usable
        .iter()
        .filter(|i| i.width.unwrap_or(0) >= 300)
        .min_by_key(|i| i.width.unwrap_or(u32::MAX))
        .or_else(|| usable.iter().max_by_key(|i| i.width.unwrap_or(0)))?;
    Some(ImageRef {
        url: chosen.url.clone(),
        headers: Vec::new(),
        width: chosen.width,
        height: chosen.height,
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Named {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExternalIds {
    #[serde(default)]
    pub isrc: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Album {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub artists: Vec<Named>,
    #[serde(default)]
    pub images: Vec<Image>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub total_tracks: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Track {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub artists: Vec<Named>,
    #[serde(default)]
    pub album: Option<Album>,
    #[serde(default)]
    pub duration_ms: Option<u32>,
    #[serde(default)]
    pub track_number: Option<u32>,
    #[serde(default)]
    pub disc_number: Option<u32>,
    #[serde(default)]
    pub explicit: bool,
    #[serde(default)]
    pub external_ids: Option<ExternalIds>,
    #[serde(default)]
    pub is_local: bool,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Artist {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub images: Vec<Image>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Owner {
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Total {
    #[serde(default)]
    pub total: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Playlist {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub owner: Option<Owner>,
    #[serde(default)]
    pub images: Option<Vec<Image>>,
    /// `items` since 2026; `tracks` before (deprecated).
    #[serde(default)]
    pub items: Option<Total>,
    #[serde(default)]
    pub tracks: Option<Total>,
}

/// A Spotify id: base62, 22 characters in practice; anything else is refused
/// so it cannot change a request's path.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn names(list: &[Named]) -> Option<String> {
    let names: Vec<&str> = list
        .iter()
        .map(|a| a.name.trim())
        .filter(|n| !n.is_empty())
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

fn year(date: Option<&str>) -> Option<u32> {
    date.and_then(|d| d.get(..4)).and_then(|y| y.parse().ok())
}

fn isrc(ids: Option<&ExternalIds>) -> Option<String> {
    let code = ids?.isrc.as_deref()?.trim().replace('-', "").to_ascii_uppercase();
    (code.len() == 12 && code.bytes().all(|b| b.is_ascii_alphanumeric())).then_some(code)
}

fn id(value: &Option<String>) -> Option<String> {
    value.as_deref().filter(|i| valid_id(i)).map(str::to_owned)
}

impl Track {
    /// The track, or `None` for local files, episodes and removed tracks.
    pub fn item(&self, album_fallback: Option<&Item>) -> Option<Item> {
        if self.is_local || self.kind.as_deref().is_some_and(|k| k != "track") {
            return None;
        }
        let mut item = Item::new(ItemKind::Track, id(&self.id)?, self.name.trim());
        if item.title.is_empty() {
            return None;
        }
        item.artist = names(&self.artists);
        item.artist_id = self.artists.first().and_then(|a| id(&a.id));
        item.duration_ms = self.duration_ms;
        item.track_number = self.track_number;
        item.disc_number = self.disc_number;
        item.explicit = self.explicit;
        item.isrc = isrc(self.external_ids.as_ref());
        match &self.album {
            Some(album) => {
                item.album = Some(album.name.clone()).filter(|n| !n.is_empty());
                item.album_id = id(&album.id);
                item.artwork = artwork(&album.images);
                item.year = year(album.release_date.as_deref());
            }
            None => {
                if let Some(album) = album_fallback {
                    item.album = Some(album.title.clone());
                    item.album_id = Some(album.id.clone());
                    item.artwork = album.artwork.clone();
                    item.year = album.year;
                }
            }
        }
        Some(item)
    }
}

impl Album {
    pub fn item(&self) -> Option<Item> {
        let mut item = Item::new(ItemKind::Album, id(&self.id)?, self.name.trim());
        if item.title.is_empty() {
            return None;
        }
        item.artist = names(&self.artists);
        item.artist_id = self.artists.first().and_then(|a| id(&a.id));
        item.artwork = artwork(&self.images);
        item.year = year(self.release_date.as_deref());
        item.child_count = self.total_tracks;
        Some(item)
    }
}

impl Artist {
    pub fn item(&self) -> Option<Item> {
        let mut item = Item::new(ItemKind::Artist, id(&self.id)?, self.name.trim());
        if item.title.is_empty() {
            return None;
        }
        item.artwork = artwork(&self.images);
        Some(item)
    }
}

impl Playlist {
    pub fn item(&self) -> Option<Item> {
        let mut item = Item::new(ItemKind::Playlist, id(&self.id)?, self.name.trim());
        if item.title.is_empty() {
            return None;
        }
        item.artist = self
            .owner
            .as_ref()
            .and_then(|o| o.display_name.clone())
            .filter(|n| !n.is_empty());
        item.artwork = self.images.as_deref().and_then(artwork);
        item.child_count = self.items.as_ref().or(self.tracks.as_ref()).and_then(|t| t.total);
        Some(item)
    }
}

/// A `spotify:<type>:<id>` URI for an item, if it has one.
pub fn uri(item: &Item) -> Option<String> {
    let kind = match item.kind {
        ItemKind::Track => "track",
        ItemKind::Album => "album",
        ItemKind::Artist => "artist",
        ItemKind::Playlist => "playlist",
        ItemKind::Folder => return None,
    };
    valid_id(&item.id).then(|| format!("spotify:{kind}:{}", item.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn track() -> Track {
        serde_json::from_value(json!({
            "id": "4uLU6hMCjMI75M1A2tKUQC", "name": "Harbour Lights", "type": "track",
            "artists": [{ "id": "0OdUWJ0sBjDrqHygGUXeCF", "name": "Ada Example" }, { "id": "x1", "name": "The Placeholders" }],
            "album": { "id": "6akEvsycLGftJxYudPjmqK", "name": "Night Ferry", "release_date": "2021-05-07",
                       "images": [ { "url": "https://i.scdn.co/image/a640", "width": 640, "height": 640 },
                                   { "url": "https://i.scdn.co/image/a300", "width": 300, "height": 300 },
                                   { "url": "https://i.scdn.co/image/a64", "width": 64, "height": 64 } ] },
            "duration_ms": 201000, "track_number": 3, "disc_number": 1, "explicit": true,
            "external_ids": { "isrc": "us-rc1-76-07839" }
        }))
        .unwrap()
    }

    #[test]
    fn tracks_keep_spotify_ids_and_details() {
        let item = track().item(None).unwrap();
        assert_eq!(item.id, "4uLU6hMCjMI75M1A2tKUQC");
        assert_eq!(item.artist.as_deref(), Some("Ada Example, The Placeholders"));
        assert_eq!(item.artist_id.as_deref(), Some("0OdUWJ0sBjDrqHygGUXeCF"));
        assert_eq!(item.album_id.as_deref(), Some("6akEvsycLGftJxYudPjmqK"));
        assert_eq!(item.isrc.as_deref(), Some("USRC17607839"));
        assert_eq!(item.year, Some(2021));
        assert_eq!(item.artwork.unwrap().url, "https://i.scdn.co/image/a300");
        assert!(item.explicit && item.playable);
        assert_eq!(
            uri(&track().item(None).unwrap()).as_deref(),
            Some("spotify:track:4uLU6hMCjMI75M1A2tKUQC")
        );
    }

    #[test]
    fn odd_entries_are_skipped() {
        let mut local = track();
        local.is_local = true;
        assert!(local.item(None).is_none());
        let mut episode = track();
        episode.kind = Some("episode".into());
        assert!(episode.item(None).is_none());
        let mut bad = track();
        bad.id = Some("../me".into());
        assert!(bad.item(None).is_none());
        let p: Playlist = serde_json::from_value(
            json!({ "id": "37i9dQZF1DX0", "name": "Mix", "images": null, "items": { "total": 12 } }),
        )
        .unwrap();
        let item = p.item().unwrap();
        assert_eq!((item.child_count, item.artwork.is_none()), (Some(12), true));
        let old: Playlist =
            serde_json::from_value(json!({ "id": "a1", "name": "Old", "tracks": { "total": 3 } })).unwrap();
        assert_eq!(old.item().unwrap().child_count, Some(3));
        assert!(uri(&Item::new(ItemKind::Folder, "tracks", "Liked Songs")).is_none());
    }
}
