//! The Jellyfesh source plugin for Jellyfin servers (10.9 and later).
//!
//! * Sign-in: Quick Connect (a code shown in Jellyfesh, approved in
//!   Jellyfin) or user name and password; only the access token is kept, in
//!   the keystore ([`login`]). Every request carries the `MediaBrowser`
//!   authorisation header with a stable random device id ([`api`]).
//! * Search: `/Items` with `searchTerm` for tracks, albums and playlists,
//!   `/Artists` for artists, each paged by the server. Browse: music
//!   libraries → artists, albums, recently added → tracks; playlists;
//!   favourites. Stream: `/Audio/{id}/universal` with the formats the
//!   engine plays and a bit-rate limit from the level the app asks for
//!   ([`media`]).
//!   Artwork: `/Items/{id}/Images/Primary` at the asked size. Lyrics:
//!   `/Audio/{id}/Lyrics`, ticks to LRC ([`lyrics`]). Recommendations:
//!   instant mix. Logout reports the end of the session to the server.
//! * Discover (API 1.2): Recently Added (`/Items/Latest`), Continue
//!   Listening (`/UserItems/Resume`), Favourites and Suggestions
//!   (`/Items/Suggestions`). Details: `Overview`, genres, release date,
//!   labels and the people on a song ([`details`]).

mod api;
mod details;
mod items;
mod login;
mod lyrics;
mod media;
mod model;
mod settings;

use jf_plugin_sdk::prelude::*;

use crate::api::Auth;
use crate::items::Id;
use crate::model::{BaseItem, ItemList};

struct Jellyfin;

/// Fields asked for in listings: whether the account may play an item, and
/// how many tracks an album or playlist has.
const FIELDS: &str = "PlayAccess,ChildCount";

/// Text for a plugin error, for people.
pub(crate) fn error_text(error: &PluginError) -> String {
    match error {
        PluginError::Unsupported => "Not supported.".into(),
        PluginError::AuthRequired => "Sign in again.".into(),
        PluginError::NotFound => "Not found.".into(),
        PluginError::InvalidInput(text)
        | PluginError::Network(text)
        | PluginError::Service(text)
        | PluginError::Internal(text) => text.clone(),
        PluginError::RateLimited => "The server asked Jellyfesh to slow down.".into(),
    }
}

/// One page of `/Items` (or another list endpoint) with server paging.
fn list(path: &str, params: &[(&str, &str)], page: &PageRequest) -> Result<ItemPage> {
    let start = items::offset(page)?;
    let limit = page.limit.clamp(1, 200);
    let (start_text, limit_text) = (start.to_string(), limit.to_string());
    let mut query = params.to_vec();
    query.extend_from_slice(&[
        ("startIndex", start_text.as_str()),
        ("limit", limit_text.as_str()),
        ("fields", FIELDS),
        ("enableImageTypes", "Primary"),
        ("imageTypeLimit", "1"),
    ]);
    let found: ItemList = api::get(path, &query)?;
    let got = found.items.len();
    let total = found.total_record_count.and_then(|t| usize::try_from(t).ok());
    let more = match total {
        Some(total) => start + got < total,
        None => got >= limit as usize,
    };
    Ok(ItemPage {
        items: items::items(&found.items),
        next: (more && got > 0).then(|| (start + got).to_string()),
        total: total.and_then(|t| u32::try_from(t).ok()),
    })
}

/// Items per Discover shelf.
const SHELF_SIZE: &str = "20";

/// Common listing parameters for a shelf.
const SHELF_PARAMS: [(&str, &str); 4] = [
    ("limit", SHELF_SIZE),
    ("fields", FIELDS),
    ("enableImageTypes", "Primary"),
    ("imageTypeLimit", "1"),
];

/// One shelf from a list endpoint: (items, or none when the server has no
/// such list). Sign-in and network problems fail the page.
fn shelf_items(path: &str, params: &[(&str, &str)], bare_array: bool) -> Result<Option<Vec<Item>>> {
    let mut query = params.to_vec();
    query.extend_from_slice(&SHELF_PARAMS);
    let response = api::send(
        http::Request::get(api::url(path)?).query(&query),
        api::Auth::Session,
    )?;
    if matches!(response.status, 400 | 404) {
        jf_plugin_sdk::warn!("no {path} for Discover: HTTP {}", response.status);
        return Ok(None);
    }
    let response = api::check(response)?;
    let list = if bare_array {
        api::json::<Vec<model::BaseItem>>(&response)?
    } else {
        api::json::<ItemList>(&response)?.items
    };
    Ok(Some(items::items(&list)))
}

/// A Discover shelf and where its items come from.
struct ShelfSource {
    id: &'static str,
    title: &'static str,
    style: ShelfStyle,
    /// The browse node of the whole list.
    more: Option<String>,
    path: &'static str,
    params: &'static [(&'static str, &'static str)],
    /// The endpoint answers a bare list instead of a `QueryResult`.
    bare: bool,
}

/// The Discover page: what the server lists for the account.
fn discover_shelves() -> Result<Vec<Shelf>> {
    let libraries = api::music_libraries()?;
    let recent_more = match libraries.as_slice() {
        [(id, _)] => Some(format!("f:lib:{id}:recent")),
        _ => None,
    };
    let sources = [
        ShelfSource {
            id: "latest",
            title: "Recently Added",
            style: ShelfStyle::Row,
            more: recent_more,
            path: "/Items/Latest",
            params: &[("includeItemTypes", "Audio"), ("groupItems", "true")],
            bare: true,
        },
        ShelfSource {
            id: "resume",
            title: "Continue Listening",
            style: ShelfStyle::Row,
            more: None,
            path: "/UserItems/Resume",
            params: &[("mediaTypes", "Audio")],
            bare: false,
        },
        ShelfSource {
            id: "favourites",
            title: "Favourites",
            style: ShelfStyle::Grid,
            more: Some("f:favourites".into()),
            path: "/Items",
            params: &[
                ("isFavorite", "true"),
                ("recursive", "true"),
                ("includeItemTypes", "MusicAlbum,Audio"),
                ("sortBy", "SortName"),
                ("sortOrder", "Ascending"),
            ],
            bare: false,
        },
        ShelfSource {
            id: "suggestions",
            title: "Suggestions",
            style: ShelfStyle::Row,
            more: None,
            path: "/Items/Suggestions",
            params: &[("mediaType", "Audio"), ("type", "MusicAlbum")],
            bare: false,
        },
    ];
    let mut shelves = Vec::new();
    for source in sources {
        let Some(found) = shelf_items(source.path, source.params, source.bare)? else {
            continue;
        };
        if found.is_empty() {
            continue;
        }
        shelves.push(Shelf {
            id: source.id.into(),
            title: source.title.into(),
            style: source.style,
            items: found,
            more: source.more,
        });
    }
    Ok(shelves)
}

fn library_sections(library: &str) -> Vec<Item> {
    vec![
        items::folder(&format!("lib:{library}:artists"), "Artists"),
        items::folder(&format!("lib:{library}:albums"), "Albums"),
        items::folder(&format!("lib:{library}:recent"), "Recently added"),
    ]
}

/// The root: the music libraries (or, with one, its sections), playlists
/// and favourites.
fn root() -> Result<ItemPage> {
    let libraries = api::music_libraries()?;
    let mut out = match libraries.as_slice() {
        [(id, _)] => library_sections(id),
        many => many
            .iter()
            .map(|(id, name)| items::folder(&format!("lib:{id}"), name))
            .collect(),
    };
    out.push(items::folder("playlists", "Playlists"));
    out.push(items::folder("favourites", "Favourites"));
    Ok(ItemPage::new(out))
}

fn browse_folder(path: &str, page: &PageRequest) -> Result<ItemPage> {
    match path {
        "playlists" => list(
            "/Items",
            &[
                ("includeItemTypes", "Playlist"),
                ("mediaTypes", "Audio"),
                ("recursive", "true"),
                ("sortBy", "SortName"),
                ("sortOrder", "Ascending"),
            ],
            page,
        ),
        "favourites" => Ok(ItemPage::new(vec![
            items::folder("favourites:songs", "Songs"),
            items::folder("favourites:albums", "Albums"),
            items::folder("favourites:artists", "Artists"),
        ])),
        "favourites:songs" | "favourites:albums" => list(
            "/Items",
            &[
                (
                    "includeItemTypes",
                    if path == "favourites:songs" {
                        "Audio"
                    } else {
                        "MusicAlbum"
                    },
                ),
                ("isFavorite", "true"),
                ("recursive", "true"),
                ("sortBy", "SortName"),
                ("sortOrder", "Ascending"),
            ],
            page,
        ),
        "favourites:artists" => list(
            "/Artists",
            &[
                ("isFavorite", "true"),
                ("sortBy", "SortName"),
                ("sortOrder", "Ascending"),
            ],
            page,
        ),
        _ => {
            let Some(rest) = path.strip_prefix("lib:") else {
                return Err(PluginError::NotFound);
            };
            let (library, section) = match rest.split_once(':') {
                Some((library, section)) => (library, Some(section)),
                None => (rest, None),
            };
            if library.is_empty() || !api::music_libraries()?.iter().any(|(id, _)| id == library) {
                return Err(PluginError::NotFound);
            }
            match section {
                None => Ok(ItemPage::new(library_sections(library))),
                Some("artists") => list(
                    "/Artists/AlbumArtists",
                    &[
                        ("parentId", library),
                        ("sortBy", "SortName"),
                        ("sortOrder", "Ascending"),
                    ],
                    page,
                ),
                Some("albums") => list(
                    "/Items",
                    &[
                        ("parentId", library),
                        ("includeItemTypes", "MusicAlbum"),
                        ("recursive", "true"),
                        ("sortBy", "SortName"),
                        ("sortOrder", "Ascending"),
                    ],
                    page,
                ),
                Some("recent") => list(
                    "/Items",
                    &[
                        ("parentId", library),
                        ("includeItemTypes", "MusicAlbum"),
                        ("recursive", "true"),
                        ("sortBy", "DateCreated,SortName"),
                        ("sortOrder", "Descending,Ascending"),
                    ],
                    page,
                ),
                Some(_) => Err(PluginError::NotFound),
            }
        }
    }
}

/// The kinds a search covers, in page order.
const KINDS: [ItemKind; 4] = [
    ItemKind::Artist,
    ItemKind::Album,
    ItemKind::Playlist,
    ItemKind::Track,
];

/// Where a search continues: an offset per kind, or `-` for a kind with no
/// more results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct SearchCursor {
    offsets: [usize; 4],
    done: [bool; 4],
}

impl SearchCursor {
    fn parse(cursor: Option<&str>) -> Result<SearchCursor> {
        let Some(cursor) = cursor.filter(|c| !c.is_empty()) else {
            return Ok(SearchCursor::default());
        };
        let bad = || PluginError::invalid_input(format!("bad cursor {cursor:?}"));
        let parts: Vec<&str> = cursor.split(',').collect();
        if parts.len() != 4 {
            return Err(bad());
        }
        let mut out = SearchCursor::default();
        for (i, part) in parts.into_iter().enumerate() {
            if part == "-" {
                out.done[i] = true;
            } else {
                out.offsets[i] = part.parse().map_err(|_| bad())?;
            }
        }
        Ok(out)
    }

    fn encode(&self) -> String {
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

/// How many artists, albums, playlists and tracks to ask for: a fifth of
/// the page for each of the first three before the tracks, or an even share
/// when tracks are not wanted.
fn allocation(wanted: [bool; 4], limit: u32) -> [u32; 4] {
    let left: Vec<usize> = (0..4).filter(|i| wanted[*i]).collect();
    let mut out = [0; 4];
    match left.as_slice() {
        [] => {}
        [only] => out[*only] = limit,
        _ if wanted[3] => {
            let small = limit / 5;
            for i in 0..3 {
                if wanted[i] {
                    out[i] = small;
                }
            }
            out[3] = limit - out[..3].iter().sum::<u32>();
        }
        _ => {
            let share = limit / left.len() as u32;
            let mut extra = limit - share * left.len() as u32;
            for &i in &left {
                out[i] = share + u32::from(extra > 0);
                extra = extra.saturating_sub(1);
            }
        }
    }
    out
}

/// One kind of search results: (items, whether there may be more).
fn search_kind(kind: ItemKind, query: &str, offset: usize, count: u32) -> Result<(Vec<Item>, bool)> {
    let (start, limit) = (offset.to_string(), count.to_string());
    let mut params = vec![
        ("searchTerm", query),
        ("startIndex", start.as_str()),
        ("limit", limit.as_str()),
        ("fields", FIELDS),
        ("enableImageTypes", "Primary"),
        ("imageTypeLimit", "1"),
    ];
    let path = match kind {
        ItemKind::Artist => "/Artists",
        _ => {
            params.push(("recursive", "true"));
            params.push((
                "includeItemTypes",
                match kind {
                    ItemKind::Album => "MusicAlbum",
                    ItemKind::Playlist => "Playlist",
                    _ => "Audio",
                },
            ));
            "/Items"
        }
    };
    let found: ItemList = api::get(path, &params)?;
    let mut list = found.items;
    list.truncate(count as usize);
    let got = list.len();
    let more = match found.total_record_count.and_then(|t| usize::try_from(t).ok()) {
        Some(total) => offset + got < total && got > 0,
        None => got >= count as usize,
    };
    Ok((items::items(&list), more))
}

/// A track of this server that matches an item from another source (by
/// title, artist and length), for lyrics and recommendations.
fn find_track(item: &Item) -> Result<Option<String>> {
    let title = item.title.trim();
    if title.is_empty() {
        return Ok(None);
    }
    let found: ItemList = api::get(
        "/Items",
        &[
            ("searchTerm", title),
            ("includeItemTypes", "Audio"),
            ("recursive", "true"),
            ("limit", "20"),
        ],
    )?;
    let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
    Ok(found
        .items
        .iter()
        .find(|candidate| {
            same(candidate.name.as_deref().unwrap_or_default(), title)
                && match &item.artist {
                    Some(wanted) => {
                        let wanted = wanted.to_lowercase();
                        candidate.artists.is_empty()
                            || candidate.artists.iter().any(|a| {
                                let a = a.to_lowercase();
                                wanted.contains(&a) || a.contains(&wanted)
                            })
                    }
                    None => true,
                }
                && match (item.duration_ms, candidate.run_time_ticks) {
                    (Some(ms), Some(ticks)) => (i64::from(ms) - ticks / items::TICKS_PER_MS).abs() <= 3000,
                    _ => true,
                }
        })
        .map(|candidate| candidate.id.clone()))
}

/// The item with `id`, with its media sources.
fn fetch_item(id: &str) -> Result<BaseItem> {
    let found: ItemList = api::get(
        "/Items",
        &[
            ("ids", id),
            ("fields", "MediaSources,PlayAccess"),
            ("enableImages", "false"),
        ],
    )?;
    found
        .items
        .into_iter()
        .find(|item| item.id.eq_ignore_ascii_case(id))
        .ok_or(PluginError::NotFound)
}

impl Plugin for Jellyfin {
    fn capabilities() -> Vec<Capability> {
        vec![
            Capability::Auth,
            Capability::Search,
            Capability::Browse,
            Capability::ResolveStream,
            Capability::Lyrics,
            Capability::Artwork,
            Capability::Recommendations,
        ]
    }

    fn settings_changed(settings: &serde_json::Value) -> Result<()> {
        api::set_settings(settings::Settings::parse(settings)?);
        Ok(())
    }

    fn auth_begin(flow: Flow) -> Result<FlowStep> {
        login::begin(flow)
    }

    fn auth_poll(flow: Flow) -> Result<FlowStep> {
        login::poll(flow)
    }

    fn auth_submit(flow: Flow, fields: Vec<FieldValue>) -> Result<FlowStep> {
        login::submit(flow, fields)
    }

    fn logout() -> Result<()> {
        // Tell the server the session ended, so the token stops working
        // there too; the host forgets it here whatever happens.
        if let Ok(Some(token)) = api::token() {
            match api::url("/Sessions/Logout")
                .and_then(|url| api::send(http::Request::post(url), Auth::Token(&token)))
            {
                Ok(response) if response.is_success() || response.status == 401 => {}
                Ok(response) => {
                    jf_plugin_sdk::warn!("the server answered HTTP {} to the logout", response.status)
                }
                Err(error) => jf_plugin_sdk::warn!("could not report the logout: {}", error_text(&error)),
            }
        }
        api::forget();
        Ok(())
    }

    fn search(query: String, filters: SearchFilters, page: PageRequest) -> Result<ItemPage> {
        let wants = |kind| filters.kinds.is_empty() || filters.kinds.contains(&kind);
        let wanted = KINDS.map(wants);
        let mut cursor = SearchCursor::parse(page.cursor.as_deref())?;
        let left = [0, 1, 2, 3].map(|i| wanted[i] && !cursor.done[i]);
        let counts = allocation(left, page.limit.clamp(1, 200));
        let query = query.trim();
        let mut results = Vec::new();
        for i in 0..4 {
            if counts[i] == 0 {
                if !wanted[i] {
                    cursor.done[i] = true;
                }
                continue;
            }
            let (found, more) = search_kind(KINDS[i], query, cursor.offsets[i], counts[i])?;
            cursor.offsets[i] += found.len();
            cursor.done[i] = !more;
            results.extend(found);
        }
        let more = (0..4).any(|i| wanted[i] && !cursor.done[i]);
        Ok(ItemPage {
            items: results,
            next: more.then(|| cursor.encode()),
            total: None,
        })
    }

    fn browse(node: Option<Item>, page: PageRequest) -> Result<ItemPage> {
        let Some(node) = node else {
            return root();
        };
        match Id::parse(&node.id) {
            Some(Id::Folder(path)) => browse_folder(path, &page),
            Some(Id::Artist(id)) => list(
                "/Items",
                &[
                    ("artistIds", id),
                    ("includeItemTypes", "MusicAlbum"),
                    ("recursive", "true"),
                    ("sortBy", "ProductionYear,SortName"),
                    ("sortOrder", "Descending,Ascending"),
                ],
                &page,
            ),
            Some(Id::Album(id)) => list(
                "/Items",
                &[
                    ("parentId", id),
                    ("includeItemTypes", "Audio"),
                    ("sortBy", "ParentIndexNumber,IndexNumber,SortName"),
                    ("sortOrder", "Ascending"),
                ],
                &page,
            ),
            Some(Id::Playlist(id)) => list(&format!("/Playlists/{}/Items", http::encode(id)), &[], &page),
            Some(Id::Track(_)) => Err(PluginError::invalid_input("a track has no children")),
            None => Err(PluginError::NotFound),
        }
    }

    fn resolve_stream(item: Item, quality: Quality) -> Result<StreamInfo> {
        let Some(Id::Track(id)) = Id::parse(&item.id) else {
            return Err(PluginError::invalid_input("not a track from this server"));
        };
        let token = api::session()?;
        let found = fetch_item(id)?;
        if let Some(reason) = items::unplayable(&found) {
            return Err(PluginError::service(
                reason
                    .message
                    .unwrap_or_else(|| "This track can't be played.".into()),
            ));
        }
        // The level the app asks for (ADR-0026); the plugin has no quality
        // setting of its own.
        let settings = api::settings();
        let plan = media::plan(found.media_sources.first(), quality, settings.codec);
        let query = media::query(&api::device_id()?, &plan, settings.codec);
        let pairs: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let url = format!(
            "{}/Audio/{}/universal?{}",
            api::base()?,
            http::encode(id),
            http::form_encode(&pairs)
        );
        let mut info = StreamInfo::new(url);
        info.headers
            .push(Header::new("authorization", api::authorization(Some(&token))?));
        info.codec = plan.codec;
        info.bitrate_kbps = plan.bitrate_kbps;
        info.cacheable = true;
        Ok(info)
    }

    fn lyrics(item: Item) -> Result<Option<LyricPayload>> {
        let id = match Id::parse(&item.id) {
            Some(Id::Track(id)) => id.to_string(),
            Some(_) => return Err(PluginError::invalid_input("lyrics are for tracks")),
            None => match find_track(&item)? {
                Some(id) => id,
                None => return Ok(None),
            },
        };
        let response = api::send(
            http::Request::get(api::url(&format!("/Audio/{}/Lyrics", http::encode(&id)))?),
            Auth::Session,
        )?;
        // No lyrics is a 404, as is a server without the lyrics API.
        if response.status == 404 {
            return Ok(None);
        }
        let found: model::Lyrics = api::json(&api::check(response)?)?;
        Ok(lyrics::payload(&found))
    }

    fn artwork(item: Item, size: u32) -> Result<Option<ImageRef>> {
        let Some(id) = Id::parse(&item.id).and_then(Id::server_id) else {
            return Ok(None);
        };
        let image = match api::image_of(&item.id) {
            Some(image) => Some(image),
            None => {
                let found: ItemList = api::get(
                    "/Items",
                    &[
                        ("ids", id),
                        ("enableImageTypes", "Primary"),
                        ("imageTypeLimit", "1"),
                    ],
                )?;
                let image = found.items.first().and_then(items::image);
                if let Some(image) = &image {
                    api::remember_image(&item.id, image);
                }
                image
            }
        };
        let Some((image_item, tag)) = image else {
            return Ok(None);
        };
        Ok(Some(ImageRef::new(items::image_url(
            &image_item,
            Some(&tag),
            size,
        )?)))
    }

    fn discover() -> Result<Vec<Shelf>> {
        discover_shelves()
    }

    fn details(item: Item) -> Result<ItemDetails> {
        details::details(&item)
    }

    fn recommendations_for_item(item: Item, page: PageRequest) -> Result<ItemPage> {
        if page.cursor.is_some() {
            // One page: an instant mix is made fresh each time.
            return Ok(ItemPage::new(Vec::new()));
        }
        let id = match Id::parse(&item.id).and_then(Id::server_id) {
            Some(id) => id.to_string(),
            None if Id::parse(&item.id).is_some() => return Err(PluginError::Unsupported),
            None => match find_track(&item)? {
                Some(id) => id,
                None => return Ok(ItemPage::new(Vec::new())),
            },
        };
        let limit = page.limit.clamp(1, 200).to_string();
        let found: ItemList = api::get(
            &format!("/Items/{}/InstantMix", http::encode(&id)),
            &[("limit", &limit), ("fields", FIELDS)],
        )?;
        let own = format!("tr:{id}");
        Ok(ItemPage::new(
            items::items(&found.items)
                .into_iter()
                .filter(|track| track.id != own && track.kind == ItemKind::Track)
                .collect(),
        ))
    }
}

jf_plugin_sdk::export_plugin!(Jellyfin);

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
    fn search_shares_the_page_between_kinds() {
        assert_eq!(allocation([true, true, true, true], 50), [10, 10, 10, 20]);
        assert_eq!(allocation([false, false, false, true], 50), [0, 0, 0, 50]);
        assert_eq!(allocation([true, false, false, false], 7), [7, 0, 0, 0]);
        assert_eq!(allocation([true, true, false, false], 7), [4, 3, 0, 0]);
        assert_eq!(allocation([true, true, true, true], 2), [0, 0, 0, 2]);
        assert_eq!(allocation([false; 4], 9), [0; 4]);
    }

    #[test]
    fn manifest_version_matches_the_crate() {
        let manifest = include_str!("../manifest.toml");
        let line = format!("version = \"{}\"", env!("CARGO_PKG_VERSION"));
        assert!(
            manifest.lines().any(|l| l == line),
            "manifest.toml must say {line}"
        );
    }
}
