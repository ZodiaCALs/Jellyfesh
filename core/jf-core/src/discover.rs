//! Discover pages (ADR-0022 §2).
//!
//! A source with a `discover` export gives its own shelves. For one
//! without, the core puts a page together from what the source has: "For
//! You" from `recommendations.for-user`, then a shelf for each of the first
//! folders, playlists and albums its browse root lists (See All browses
//! them). The library's page has Recently Added, Most Played and
//! Rediscover (songs played often, but not for a while). Each source's page
//! holds only its own items, as its content policy asks; items the service
//! will not play here stay marked unplayable.

use std::collections::HashSet;
use std::time::Duration;

use jf_library::{Library, PlaylistItem};
use jf_plugin_host::{self as host, CallError, Capability};

use crate::sources::{Ctx, LIBRARY, source_item};
use crate::types::{CommandId, DiscoverPage, ItemKind, Shelf, ShelfStyle, SourceItem};

/// How long one plugin call for a Discover page may take.
const CALL_WAIT: Duration = Duration::from_secs(25);
/// Items on a shelf the core puts together.
const SHELF_ITEMS: u32 = 20;
/// Root folders made into shelves.
const ROOT_SHELVES: usize = 6;
/// Songs not played for this long can be rediscovered.
const REDISCOVER_AFTER_MS: i64 = 60 * 24 * 60 * 60 * 1000;

fn wait<T>(reply: &host::Reply<T>) -> Result<T, CallError> {
    reply.recv_timeout(CALL_WAIT).unwrap_or(Err(CallError::Timeout))
}

fn shelf_style(style: host::ShelfStyle) -> ShelfStyle {
    match style {
        host::ShelfStyle::Row => ShelfStyle::Row,
        host::ShelfStyle::Grid => ShelfStyle::Grid,
        host::ShelfStyle::Chart => ShelfStyle::Chart,
    }
}

/// A source's Discover page: its own shelves, or a page made from its
/// recommendations and browse root. Blocks on the plugin's calls. The items
/// are remembered, so playing them and See All pass them back whole.
pub(crate) fn source_page(ctx: &Ctx, plugin: &str, request: CommandId) -> DiscoverPage {
    let host = &ctx.host;
    let links = ctx.links(plugin);
    let mut page = DiscoverPage {
        request,
        source: plugin.to_owned(),
        shelves: Vec::new(),
        error: None,
    };
    let Some(info) = host.plugin(plugin) else {
        page.error = Some(format!("the source {plugin} is not installed"));
        return page;
    };
    let rows = |items: &[host::Item]| -> Vec<SourceItem> {
        ctx.remember(plugin, items);
        items.iter().map(|i| source_item(plugin, i, &links)).collect()
    };
    if info.capabilities.contains(&Capability::Discover) {
        match wait(&host.discover(plugin)) {
            Ok(shelves) => {
                page.shelves = shelves
                    .into_iter()
                    .filter(|s| !s.items.is_empty())
                    .map(|s| Shelf {
                        id: s.id,
                        title: s.title,
                        style: shelf_style(s.style),
                        items: rows(&s.items),
                        more: s.more,
                    })
                    .collect();
                return page;
            }
            Err(e) => tracing::info!(%plugin, error = %e, "no Discover page from the plugin; making one"),
        }
    }
    let mut errors = Vec::new();
    let first = |limit| host::PageRequest { cursor: None, limit };
    if info.capabilities.contains(&Capability::Recommendations) {
        match wait(&host.recommendations_for_user(plugin, first(SHELF_ITEMS))) {
            Ok(found) if !found.items.is_empty() => page.shelves.push(Shelf {
                id: "for_you".into(),
                title: "For You".into(),
                style: ShelfStyle::Row,
                items: rows(&found.items),
                more: None,
            }),
            Ok(_) => {}
            Err(e) => errors.push(e.to_string()),
        }
    }
    if info.capabilities.contains(&Capability::Browse) {
        match wait(&host.browse(plugin, None, first(50))) {
            Ok(root) => {
                ctx.remember(plugin, &root.items);
                let (folders, loose): (Vec<host::Item>, Vec<host::Item>) = root
                    .items
                    .into_iter()
                    .partition(|i| matches!(i.kind, host::ItemKind::Folder | host::ItemKind::Playlist));
                for folder in folders.into_iter().take(ROOT_SHELVES) {
                    match wait(&host.browse(plugin, Some(folder.clone()), first(SHELF_ITEMS))) {
                        Ok(found) if !found.items.is_empty() => page.shelves.push(Shelf {
                            id: folder.id.clone(),
                            title: folder.title.clone(),
                            style: ShelfStyle::Row,
                            items: rows(&found.items),
                            more: Some(folder.id),
                        }),
                        Ok(_) => {}
                        Err(e) => errors.push(e.to_string()),
                    }
                }
                if !loose.is_empty() {
                    page.shelves.push(Shelf {
                        id: "browse".into(),
                        title: info.name.clone(),
                        style: ShelfStyle::Grid,
                        items: rows(&loose),
                        more: None,
                    });
                }
            }
            Err(e) => errors.push(e.to_string()),
        }
    }
    if page.shelves.is_empty() {
        page.error = Some(
            errors
                .into_iter()
                .next()
                .unwrap_or_else(|| format!("{} has nothing to discover", info.name)),
        );
    }
    page
}

fn track_row(track: &jf_library::Track) -> SourceItem {
    SourceItem {
        source: LIBRARY.into(),
        kind: ItemKind::Track,
        id: track.id.to_string(),
        title: track.title.clone(),
        subtitle: track.artist.clone(),
        album: track.album.clone(),
        duration_ms: track.duration_ms,
        artwork: None,
        playable: true,
        unplayable_reason: None,
        track: Some(jf_session::TrackRef::Library(track.id)),
        also_in: Vec::new(),
        link: None,
        artist_id: None,
        album_id: None,
    }
}

fn album_row(album: &jf_library::Album) -> SourceItem {
    SourceItem {
        source: LIBRARY.into(),
        kind: ItemKind::Album,
        id: album.id.to_string(),
        title: album.title.clone(),
        subtitle: album.album_artist.clone(),
        album: None,
        duration_ms: None,
        artwork: None,
        playable: true,
        unplayable_reason: None,
        track: None,
        also_in: Vec::new(),
        link: None,
        artist_id: None,
        album_id: None,
    }
}

/// The library's Discover page.
pub(crate) fn library_page(library: &Library, request: CommandId, now_ms: i64) -> DiscoverPage {
    let mut shelves = Vec::new();
    let mut error = None;
    // Recently Added: the albums of the newest songs, newest first.
    match library.recently_added(200) {
        Ok(tracks) => {
            let mut seen = HashSet::new();
            let mut items = Vec::new();
            for track in &tracks {
                let row = match track.album_id {
                    Some(album) if seen.insert(album) => match library.album(album) {
                        Ok(Some(album)) => album_row(&album),
                        _ => continue,
                    },
                    Some(_) => continue,
                    None => track_row(track),
                };
                items.push(row);
                if items.len() == SHELF_ITEMS as usize {
                    break;
                }
            }
            if !items.is_empty() {
                shelves.push(Shelf {
                    id: "recently_added".into(),
                    title: "Recently Added".into(),
                    style: ShelfStyle::Row,
                    items,
                    more: None,
                });
            }
        }
        Err(e) => error = Some(e.to_string()),
    }
    match library.most_played(200) {
        Ok(stats) => {
            let library_track = |s: &jf_library::ItemStats| match (&s.item, &s.track) {
                (PlaylistItem::Library(_), Some(track)) | (PlaylistItem::File(_), Some(track)) => {
                    Some(track.clone())
                }
                _ => None,
            };
            let most: Vec<SourceItem> = stats
                .iter()
                .filter(|s| s.play_count > 0)
                .filter_map(library_track)
                .take(SHELF_ITEMS as usize)
                .map(|t| track_row(&t))
                .collect();
            // Rediscover: played at least twice, but not in the last two months.
            let rediscover: Vec<SourceItem> = stats
                .iter()
                .filter(|s| s.play_count >= 2)
                .filter(|s| {
                    s.last_played_ms
                        .is_some_and(|at| now_ms - at > REDISCOVER_AFTER_MS)
                })
                .filter_map(library_track)
                .map(|t| track_row(&t))
                .take(SHELF_ITEMS as usize)
                .collect();
            if !most.is_empty() {
                shelves.push(Shelf {
                    id: "most_played".into(),
                    title: "Most Played".into(),
                    style: ShelfStyle::Chart,
                    items: most,
                    more: None,
                });
            }
            if !rediscover.is_empty() {
                shelves.push(Shelf {
                    id: "rediscover".into(),
                    title: "Rediscover".into(),
                    style: ShelfStyle::Row,
                    items: rediscover,
                    more: None,
                });
            }
        }
        Err(e) => error = error.or(Some(e.to_string())),
    }
    DiscoverPage {
        request,
        source: LIBRARY.into(),
        shelves,
        error,
    }
}
