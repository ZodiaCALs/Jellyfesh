//! Spotify for Jellyfesh (beta): remote control of the person's own Spotify
//! devices through the Web API, with the Client ID of their own Spotify app
//! (Development Mode). Search, browse (playlists, saved albums, liked songs,
//! albums, artists) and the player; Spotify plays the music itself, never
//! Jellyfesh's engine (ADR-0011, ADR-0018).
//!
//! Tested only against a mock server written from the Web API reference,
//! not yet against the live service.

mod api;
mod items;
mod login;
mod remote;

use jf_plugin_sdk::prelude::*;
use serde::Deserialize;
use serde_json::Value;

use api::get_json;
use items::{Album, Artist, Playlist, Track, valid_id};

struct Spotify;

/// The setting only test packages have (jf-plugin-testkit's
/// `with_test_server_setting`): a mock server to use instead of Spotify.
/// Published packages' schemas do not name it, so the host never passes it.
const TEST_SERVER: &str = "test_server";

/// Most items Spotify gives per search type and request.
const SEARCH_MAX: u32 = 10;
/// Most items per page of the library and of album or playlist tracks.
const PAGE_MAX: u32 = 50;
/// Most items per page of an artist's albums.
const ARTIST_ALBUMS_MAX: u32 = 10;
/// Spotify's last search offset.
const SEARCH_OFFSET_MAX: u32 = 1000;

/// A Web API page.
#[derive(Debug, Deserialize)]
struct Paging<T> {
    #[serde(default = "Vec::new")]
    items: Vec<Option<T>>,
    #[serde(default)]
    next: Option<String>,
    #[serde(default)]
    total: Option<u32>,
}

impl<T> Default for Paging<T> {
    fn default() -> Self {
        Paging {
            items: Vec::new(),
            next: None,
            total: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct SavedAlbum {
    #[serde(default)]
    album: Option<Album>,
}

#[derive(Debug, Deserialize)]
struct SavedTrack {
    #[serde(default)]
    track: Option<Track>,
}

/// A playlist entry: `item` since 2026, `track` before (deprecated).
#[derive(Debug, Deserialize)]
struct PlaylistEntry {
    #[serde(default)]
    item: Option<Track>,
    #[serde(default)]
    track: Option<Track>,
}

fn offset(page: &PageRequest) -> Result<u32> {
    match page.cursor.as_deref() {
        Some(text) => text
            .parse()
            .map_err(|_| PluginError::invalid_input(format!("not a page cursor: {text:?}"))),
        None => Ok(0),
    }
}

/// A host page from a Web API page that started at `offset`.
fn page_of<T>(paging: Paging<T>, offset: u32, limit: u32, item: impl Fn(&T) -> Option<Item>) -> ItemPage {
    let items: Vec<Item> = paging.items.iter().flatten().filter_map(item).collect();
    ItemPage {
        items,
        next: paging.next.is_some().then(|| (offset + limit).to_string()),
        total: paging.total,
    }
}

fn library(path: &str, page: &PageRequest, max: u32) -> Result<(String, u32, u32)> {
    let offset = offset(page)?;
    let limit = page.limit.clamp(1, max);
    let separator = if path.contains('?') { '&' } else { '?' };
    Ok((
        format!("{path}{separator}limit={limit}&offset={offset}"),
        offset,
        limit,
    ))
}

/// Adds the items of one search type (null entries skipped) and notes
/// whether it has more.
fn gather<T>(
    items: &mut Vec<Item>,
    more: &mut bool,
    paging: Option<Paging<T>>,
    item: impl Fn(&T) -> Option<Item>,
) {
    let paging = paging.unwrap_or_default();
    *more |= paging.next.is_some();
    items.extend(paging.items.iter().flatten().filter_map(item));
}

fn folder(id: &str, title: &str) -> Item {
    Item::new(ItemKind::Folder, id, title)
}

impl Plugin for Spotify {
    fn capabilities() -> Vec<Capability> {
        // `remote` is a manifest capability; the host adds it.
        vec![Capability::Auth, Capability::Search, Capability::Browse]
    }

    fn settings_changed(settings: &Value) -> Result<()> {
        api::set_server(
            settings
                .get(TEST_SERVER)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty()),
        );
        Ok(())
    }

    fn auth_begin(flow: Flow) -> Result<FlowStep> {
        login::begin(flow)
    }

    fn auth_submit(flow: Flow, fields: Vec<FieldValue>) -> Result<FlowStep> {
        login::submit(flow, fields)
    }

    fn logout() -> Result<()> {
        // Spotify has no revocation for PKCE tokens; the host deletes the
        // stored Client ID and tokens.
        Ok(())
    }

    fn search(query: String, filters: SearchFilters, page: PageRequest) -> Result<ItemPage> {
        let query = query.trim().to_owned();
        let kinds: Vec<(ItemKind, &str)> = [
            (ItemKind::Track, "track"),
            (ItemKind::Artist, "artist"),
            (ItemKind::Album, "album"),
            (ItemKind::Playlist, "playlist"),
        ]
        .into_iter()
        .filter(|(kind, _)| filters.kinds.is_empty() || filters.kinds.contains(kind))
        .collect();
        if query.is_empty() || kinds.is_empty() {
            return Ok(ItemPage::new(Vec::new()));
        }
        let offset = offset(&page)?;
        if offset > SEARCH_OFFSET_MAX {
            return Ok(ItemPage::new(Vec::new()));
        }
        // Each type returns up to `limit`: share the host's page between them.
        let limit = (page.limit / kinds.len() as u32).clamp(1, SEARCH_MAX);
        let types: Vec<&str> = kinds.iter().map(|(_, t)| *t).collect();
        let path = format!(
            "/search?{}",
            http::form_encode(&[
                ("q", &query),
                ("type", &types.join(",")),
                ("limit", &limit.to_string()),
                ("offset", &offset.to_string()),
            ])
        );
        #[derive(Deserialize)]
        struct Found {
            #[serde(default)]
            tracks: Option<Paging<Track>>,
            #[serde(default)]
            artists: Option<Paging<Artist>>,
            #[serde(default)]
            albums: Option<Paging<Album>>,
            #[serde(default)]
            playlists: Option<Paging<Playlist>>,
        }
        let found: Found = get_json(&path)?;
        let mut items = Vec::new();
        let mut more = false;
        gather(&mut items, &mut more, found.tracks, |t| t.item(None));
        gather(&mut items, &mut more, found.artists, Artist::item);
        gather(&mut items, &mut more, found.albums, Album::item);
        gather(&mut items, &mut more, found.playlists, Playlist::item);
        let next = offset + limit;
        Ok(ItemPage {
            items,
            next: (more && next <= SEARCH_OFFSET_MAX).then(|| next.to_string()),
            total: None,
        })
    }

    fn browse(node: Option<Item>, page: PageRequest) -> Result<ItemPage> {
        let Some(node) = node else {
            return Ok(ItemPage::new(vec![
                folder("playlists", "Your Playlists"),
                folder("albums", "Saved Albums"),
                folder("tracks", "Liked Songs"),
            ]));
        };
        match node.kind {
            ItemKind::Folder => match node.id.as_str() {
                "playlists" => {
                    let (path, offset, limit) = library("/me/playlists", &page, PAGE_MAX)?;
                    Ok(page_of(
                        get_json::<Paging<Playlist>>(&path)?,
                        offset,
                        limit,
                        Playlist::item,
                    ))
                }
                "albums" => {
                    let (path, offset, limit) = library("/me/albums", &page, PAGE_MAX)?;
                    Ok(page_of(
                        get_json::<Paging<SavedAlbum>>(&path)?,
                        offset,
                        limit,
                        |s| s.album.as_ref().and_then(Album::item),
                    ))
                }
                "tracks" => {
                    let (path, offset, limit) = library("/me/tracks", &page, PAGE_MAX)?;
                    Ok(page_of(
                        get_json::<Paging<SavedTrack>>(&path)?,
                        offset,
                        limit,
                        |s| s.track.as_ref().and_then(|t| t.item(None)),
                    ))
                }
                _ => Err(PluginError::NotFound),
            },
            _ if !valid_id(&node.id) => Err(PluginError::NotFound),
            ItemKind::Playlist => {
                let (path, offset, limit) = library(
                    &format!("/playlists/{}/items?additional_types=track", node.id),
                    &page,
                    PAGE_MAX,
                )?;
                Ok(page_of(
                    get_json::<Paging<PlaylistEntry>>(&path)?,
                    offset,
                    limit,
                    |e| e.item.as_ref().or(e.track.as_ref()).and_then(|t| t.item(None)),
                ))
            }
            ItemKind::Album => {
                let (path, offset, limit) = library(&format!("/albums/{}/tracks", node.id), &page, PAGE_MAX)?;
                Ok(page_of(get_json::<Paging<Track>>(&path)?, offset, limit, |t| {
                    t.item(Some(&node))
                }))
            }
            ItemKind::Artist => {
                let (path, offset, limit) = library(
                    &format!("/artists/{}/albums?include_groups=album,single", node.id),
                    &page,
                    ARTIST_ALBUMS_MAX,
                )?;
                Ok(page_of(
                    get_json::<Paging<Album>>(&path)?,
                    offset,
                    limit,
                    Album::item,
                ))
            }
            ItemKind::Track => Err(PluginError::invalid_input("a track has nothing to browse")),
        }
    }

    fn remote_devices() -> Result<Vec<RemoteDevice>> {
        remote::devices()
    }

    fn remote_state() -> Result<Option<RemoteState>> {
        remote::state()
    }

    fn remote_play(
        device: Option<String>,
        context: Option<Item>,
        tracks: Vec<Item>,
        start: u32,
    ) -> Result<()> {
        remote::play(device, context, tracks, start)
    }

    fn remote_resume() -> Result<()> {
        remote::simple(http::Method::Put, "/me/player/play")
    }

    fn remote_pause() -> Result<()> {
        remote::simple(http::Method::Put, "/me/player/pause")
    }

    fn remote_next() -> Result<()> {
        remote::simple(http::Method::Post, "/me/player/next")
    }

    fn remote_previous() -> Result<()> {
        remote::simple(http::Method::Post, "/me/player/previous")
    }

    fn remote_seek(position_ms: u32) -> Result<()> {
        remote::simple(
            http::Method::Put,
            &format!("/me/player/seek?position_ms={position_ms}"),
        )
    }

    fn remote_set_volume(percent: u32) -> Result<()> {
        remote::simple(
            http::Method::Put,
            &format!("/me/player/volume?volume_percent={}", percent.min(100)),
        )
    }

    fn remote_set_shuffle(on: bool) -> Result<()> {
        remote::simple(http::Method::Put, &format!("/me/player/shuffle?state={on}"))
    }

    fn remote_set_repeat(mode: RemoteRepeat) -> Result<()> {
        let state = match mode {
            RemoteRepeat::Off => "off",
            RemoteRepeat::All => "context",
            RemoteRepeat::One => "track",
        };
        remote::simple(http::Method::Put, &format!("/me/player/repeat?state={state}"))
    }

    fn remote_transfer(device: String, play: bool) -> Result<()> {
        remote::transfer(device, play)
    }
}

jf_plugin_sdk::export_plugin!(Spotify);
