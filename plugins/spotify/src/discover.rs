//! Discover and details (plugin API 1.2, ADR-0022), from the Web API
//! endpoints a Development Mode app may still call. The Web API reference's
//! OpenAPI schema (2026) marks deprecated, among others: new releases,
//! featured playlists, categories, recommendations, related artists and an
//! artist's top tracks; an album's label, genres and popularity; an artist's
//! genres, followers and popularity. None of those is used.
//!
//! * Discover: Recently Played (`/me/player/recently-played`), Your Top
//!   Tracks and Your Top Artists of the last four weeks (`/me/top/…` with
//!   `time_range=short_term`), Your Playlists and Saved Albums. A shelf
//!   Spotify refuses is left out: a sign-in from before Jellyfesh asked for
//!   the scopes `user-read-recently-played` and `user-top-read` gets 403
//!   there until the person signs in again.
//! * Details: an album's release date, type and copyright lines, a song's
//!   release date and ISRC, a playlist's description and owner, and the
//!   largest picture of each. Spotify has no biographies and no comments.

use jf_plugin_sdk::prelude::*;
use serde::Deserialize;

use crate::api::get_json;
use crate::items::{Album, Artist, Image, Owner, Playlist, Track, valid_id};
use crate::{Paging, SavedAlbum};

/// Items on a shelf of the person's own lists.
const SHELF: u32 = 20;
/// Most plays Spotify returns from the history.
const HISTORY: u32 = 50;

/// One entry of the listening history.
#[derive(Debug, Deserialize)]
struct Play {
    #[serde(default)]
    track: Option<Track>,
}

/// The history: a cursor page of plays, newest first.
#[derive(Debug, Default, Deserialize)]
struct History {
    #[serde(default)]
    items: Vec<Option<Play>>,
}

fn shelf(id: &str, title: &str, style: ShelfStyle, items: Vec<Item>, more: Option<&str>) -> Shelf {
    Shelf {
        id: id.into(),
        title: title.into(),
        style,
        items,
        more: more.map(str::to_owned),
    }
}

/// The songs played most recently, each once.
fn recently_played() -> Result<Vec<Item>> {
    let history: History = get_json(&format!("/me/player/recently-played?limit={HISTORY}"))?;
    let mut seen = std::collections::HashSet::new();
    Ok(history
        .items
        .into_iter()
        .flatten()
        .filter_map(|play| play.track?.item(None))
        .filter(|item| seen.insert(item.id.clone()))
        .collect())
}

fn top<T: for<'de> Deserialize<'de>>(kind: &str, item: impl Fn(&T) -> Option<Item>) -> Result<Vec<Item>> {
    let page: Paging<T> = get_json(&format!("/me/top/{kind}?time_range=short_term&limit={SHELF}"))?;
    Ok(page.items.iter().flatten().filter_map(item).collect())
}

/// The Discover page.
pub fn discover() -> Result<Vec<Shelf>> {
    type Fetch = fn() -> Result<Vec<Item>>;
    let lists: [(&str, &str, ShelfStyle, Option<&str>, Fetch); 5] = [
        (
            "recent",
            "Recently Played",
            ShelfStyle::Row,
            None,
            recently_played,
        ),
        ("top-tracks", "Your Top Tracks", ShelfStyle::Chart, None, || {
            top::<Track>("tracks", |t| t.item(None))
        }),
        ("top-artists", "Your Top Artists", ShelfStyle::Row, None, || {
            top::<Artist>("artists", Artist::item)
        }),
        (
            "playlists",
            "Your Playlists",
            ShelfStyle::Grid,
            Some("playlists"),
            || {
                let page: Paging<Playlist> = get_json(&format!("/me/playlists?limit={SHELF}"))?;
                Ok(page.items.iter().flatten().filter_map(Playlist::item).collect())
            },
        ),
        ("albums", "Saved Albums", ShelfStyle::Grid, Some("albums"), || {
            let page: Paging<SavedAlbum> = get_json(&format!("/me/albums?limit={SHELF}"))?;
            Ok(page
                .items
                .iter()
                .flatten()
                .filter_map(|s| s.album.as_ref().and_then(Album::item))
                .collect())
        }),
    ];
    let mut shelves = Vec::new();
    let mut refused = None;
    for (id, title, style, more, fetch) in lists {
        match fetch() {
            Ok(items) if items.is_empty() => {}
            Ok(items) => shelves.push(shelf(id, title, style, items, more)),
            // The sign-in, the rate limit and the network concern every shelf.
            Err(error @ (PluginError::AuthRequired | PluginError::RateLimited | PluginError::Network(_))) => {
                return Err(error);
            }
            Err(error) => {
                jf_plugin_sdk::warn!("no {title} for Discover: {error:?}");
                refused.get_or_insert(error);
            }
        }
    }
    match refused {
        // Spotify refused every list (an account the app has no seat for).
        Some(error) if shelves.is_empty() => Err(error),
        _ => Ok(shelves),
    }
}

/// The largest picture (for the details view).
fn largest(images: &[Image]) -> Option<ImageRef> {
    let image = images
        .iter()
        .filter(|i| i.url.starts_with("https://"))
        .max_by_key(|i| i.width.unwrap_or(0))?;
    Some(ImageRef {
        url: image.url.clone(),
        headers: Vec::new(),
        width: image.width,
        height: image.height,
    })
}

fn fact(key: FactKey, value: Option<&str>) -> Option<Fact> {
    let value = value?.trim();
    (!value.is_empty()).then(|| Fact {
        key,
        value: value.to_owned(),
    })
}

#[derive(Debug, Default, Deserialize)]
struct Copyright {
    #[serde(default)]
    text: String,
    /// `C` (the copyright) or `P` (the sound recording).
    #[serde(rename = "type", default)]
    kind: String,
}

#[derive(Debug, Default, Deserialize)]
struct FullAlbum {
    #[serde(flatten)]
    album: Album,
    #[serde(default)]
    album_type: Option<String>,
    #[serde(default)]
    copyrights: Vec<Copyright>,
}

#[derive(Debug, Default, Deserialize)]
struct FullPlaylist {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    images: Option<Vec<Image>>,
    #[serde(default)]
    owner: Option<Owner>,
}

/// "© 2021 Example Records" for a `C` line, "℗ …" for a `P` line, unless the
/// text already starts with a sign.
fn copyright(line: &Copyright) -> Option<String> {
    let text = line.text.trim();
    if text.is_empty() {
        return None;
    }
    let signed = ["©", "℗", "(C)", "(P)", "(c)", "(p)"]
        .iter()
        .any(|sign| text.starts_with(sign));
    Some(match line.kind.as_str() {
        _ if signed => text.to_owned(),
        "C" => format!("© {text}"),
        "P" => format!("℗ {text}"),
        _ => text.to_owned(),
    })
}

fn album_type(kind: &str) -> Option<&'static str> {
    match kind {
        "album" => Some("Album"),
        "single" => Some("Single"),
        "compilation" => Some("Compilation"),
        _ => None,
    }
}

fn album_details(id: &str) -> Result<ItemDetails> {
    let full: FullAlbum = get_json(&format!("/albums/{id}"))?;
    let mut facts: Vec<Fact> = [
        fact(FactKey::Released, full.album.release_date.as_deref()),
        fact(
            FactKey::Other("Type".into()),
            full.album_type.as_deref().and_then(album_type),
        ),
    ]
    .into_iter()
    .flatten()
    .collect();
    facts.extend(full.copyrights.iter().filter_map(copyright).map(|value| Fact {
        key: FactKey::Other("Copyright".into()),
        value,
    }));
    Ok(ItemDetails {
        description: None,
        image: largest(&full.album.images),
        facts,
        comment_count: None,
    })
}

fn track_details(id: &str) -> Result<ItemDetails> {
    let track: Track = get_json(&format!("/tracks/{id}"))?;
    let album = track.album.unwrap_or_default();
    let isrc = track.external_ids.and_then(|ids| ids.isrc);
    Ok(ItemDetails {
        description: None,
        image: largest(&album.images),
        facts: [
            fact(FactKey::Released, album.release_date.as_deref()),
            fact(FactKey::Other("ISRC".into()), isrc.as_deref()),
        ]
        .into_iter()
        .flatten()
        .collect(),
        comment_count: None,
    })
}

fn artist_details(id: &str) -> Result<ItemDetails> {
    let artist: Artist = get_json(&format!("/artists/{id}"))?;
    Ok(ItemDetails {
        description: None,
        image: largest(&artist.images),
        facts: Vec::new(),
        comment_count: None,
    })
}

fn playlist_details(id: &str) -> Result<ItemDetails> {
    let playlist: FullPlaylist = get_json(&format!(
        "/playlists/{id}?{}",
        http::form_encode(&[("fields", "description,images,owner(display_name)")])
    ))?;
    let owner = playlist.owner.and_then(|o| o.display_name);
    Ok(ItemDetails {
        description: playlist.description.as_deref().and_then(plain_text),
        image: playlist.images.as_deref().and_then(largest),
        facts: fact(FactKey::Other("Owner".into()), owner.as_deref())
            .into_iter()
            .collect(),
        comment_count: None,
    })
}

/// Details of a track, album, artist or playlist.
pub fn details(item: Item) -> Result<ItemDetails> {
    if !valid_id(&item.id) {
        return Err(PluginError::NotFound);
    }
    match item.kind {
        ItemKind::Album => album_details(&item.id),
        ItemKind::Track => track_details(&item.id),
        ItemKind::Artist => artist_details(&item.id),
        ItemKind::Playlist => playlist_details(&item.id),
        ItemKind::Folder => Err(PluginError::Unsupported),
    }
}

/// Plain text from a playlist description, which Spotify sends as HTML
/// (escaped characters, sometimes links): tags dropped, entities decoded,
/// spaces collapsed. `None` when nothing is left.
pub fn plain_text(html: &str) -> Option<String> {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        text.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>') else {
            rest = &rest[open..];
            break;
        };
        rest = &rest[open + close + 1..];
    }
    text.push_str(rest);
    let decoded = decode_entities(&text);
    let out = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
    (!out.is_empty()).then_some(out)
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let end = after.find(';').filter(|e| *e <= 8);
        let decoded = end.and_then(|e| match &after[..e] {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            name => name
                .strip_prefix("#x")
                .or_else(|| name.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| name.strip_prefix('#').and_then(|n| n.parse().ok()))
                .and_then(char::from_u32),
        });
        match (decoded, end) {
            (Some(c), Some(e)) => {
                out.push(c);
                rest = &after[e + 1..];
            }
            _ => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn descriptions_lose_their_html() {
        assert_eq!(
            plain_text("Songs for a ferry &amp; a <a href=\"spotify:playlist:x\">harbour</a>&#x27;s lights.")
                .as_deref(),
            Some("Songs for a ferry & a harbour's lights.")
        );
        assert_eq!(plain_text("  <b></b> "), None);
        assert_eq!(
            plain_text("A &unknown; sign &"),
            Some("A &unknown; sign &".into())
        );
    }

    #[test]
    fn copyright_lines_get_their_sign_once() {
        let line = |text: &str, kind: &str| Copyright {
            text: text.into(),
            kind: kind.into(),
        };
        assert_eq!(
            copyright(&line("2021 Example Records", "C")).as_deref(),
            Some("© 2021 Example Records")
        );
        assert_eq!(
            copyright(&line("2021 Example Records", "P")).as_deref(),
            Some("℗ 2021 Example Records")
        );
        assert_eq!(
            copyright(&line("(P) 2021 Example", "P")).as_deref(),
            Some("(P) 2021 Example")
        );
        assert_eq!(copyright(&line("© Someone", "C")).as_deref(), Some("© Someone"));
        assert_eq!(copyright(&line("  ", "C")), None);
    }

    #[test]
    fn the_largest_https_picture_is_the_details_picture() {
        let images: Vec<Image> = serde_json::from_value(json!([
            { "url": "https://i.scdn.co/image/a300", "width": 300, "height": 300 },
            { "url": "http://i.scdn.co/image/a1000", "width": 1000, "height": 1000 },
            { "url": "https://i.scdn.co/image/a640", "width": 640, "height": 640 }
        ]))
        .unwrap();
        assert_eq!(largest(&images).unwrap().url, "https://i.scdn.co/image/a640");
        assert!(largest(&[]).is_none());
    }

    #[test]
    fn album_types_are_words() {
        assert_eq!(album_type("single"), Some("Single"));
        assert_eq!(album_type("appears_on"), None);
    }
}
