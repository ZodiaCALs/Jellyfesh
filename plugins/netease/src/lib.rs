//! NetEase Cloud Music for Jellyfesh: the open platform with the person's own
//! individual-developer credentials (App ID and RSA private key) and a QR
//! sign-in with the NetEase Cloud Music app. The account's playlists
//! (created, collected) and liked songs, their songs, and streams by
//! quality, with NetEase's rules on what each account may play: VIP songs
//! and digital albums are paywalled, previews are not played. API 1.2: a
//! Discover page of the same lists, and details of playlists. API 1.3: the
//! liked songs and the created and collected playlists for the app's
//! Favourites and Playlists pages. Streams use the level the app asks for;
//! the plugin has no quality setting of its own (ADR-0026).
//!
//! Search (ADR-0027 §5): songs, albums, artists and playlists, on the paths
//! that independent public clients of the open platform call (NetEase's
//! official client documents `ncm-cli search song --keyword`). An album
//! opens to its songs, an artist to their popular songs. An answer the
//! plugin cannot read is an error in plain words, never an empty page, and
//! its shape (keys and types, never values) goes to the log.
//!
//! Recommendations, comments and lyrics wait for NetEase's API
//! documentation: their paths were not in what the research could read (the
//! official client gets them from a server-side list of commands).
//! Tested only against a mock server written from the research, not yet
//! against the live service.

mod api;
mod items;
mod login;

use jf_plugin_sdk::prelude::*;
use serde_json::{Value, json};

use items::Access;

struct NetEase;

/// The setting only test packages have (jf-plugin-testkit's
/// `with_test_server_setting`): a mock server to use instead of NetEase.
/// Published packages' schemas do not name it, so the host never passes it.
const TEST_SERVER: &str = "test_server";

/// Most playlists or songs per request.
const PAGE_MAX: u32 = 100;

/// The account's two playlist lists, by folder id.
const LISTS: [(&str, &str, &str); 2] = [
    (
        "created",
        "Created Playlists",
        "/openapi/music/basic/playlist/created/get/v2",
    ),
    (
        "collected",
        "Collected Playlists",
        "/openapi/music/basic/playlist/subed/get/v2",
    ),
];
/// The folder of the liked songs (the songs of NetEase's red-heart playlist).
const LIKED: (&str, &str) = ("starred", "Liked Songs");
/// The liked songs' playlist: one playlist, not a list of them.
const STAR: &str = "/openapi/music/basic/playlist/star/get/v2";
const PLAYLIST: &str = "/openapi/music/basic/playlist/detail/get/v2";
const SONGS: &str = "/openapi/music/basic/playlist/song/list/get/v3";
const DETAIL: &str = "/openapi/music/basic/song/detail/get/v2";
/// An album's songs, with `albumId`, `limit` and `offset` as public clients
/// send them.
const ALBUM_SONGS: &str = "/openapi/music/basic/album/song/list/get/v2";
/// An artist's popular songs. Public clients name the path but not its
/// parameter; `artistId` is the one they send to the artist's details
/// (`artist/detail/get/v2`).
const ARTIST_SONGS: &str = "/openapi/music/basic/artists/hotsong/list/get/v2";
/// Items on a Discover shelf.
const SHELF: u32 = 20;

/// How a search record reads as an item.
type Reader = fn(&Value) -> Option<Item>;

/// The searches by kind, as independent public clients of the open platform
/// call them: `keyword`, `limit` and `offset`, all three required (without
/// `offset` NetEase answers HTTP 400). Each with what its answer lists, for
/// errors and the log.
const SEARCHES: [(ItemKind, &str, &str, Reader); 4] = [
    (
        ItemKind::Track,
        "/openapi/music/basic/search/song/get/v3",
        "the songs it found",
        items::song,
    ),
    (
        ItemKind::Album,
        "/openapi/music/basic/search/album/get/v2",
        "the albums it found",
        items::album,
    ),
    (
        ItemKind::Artist,
        "/openapi/music/basic/search/artists/get/v2",
        "the artists it found",
        items::artist,
    ),
    (
        ItemKind::Playlist,
        "/openapi/music/basic/search/playlist/get/v2",
        "the playlists it found",
        items::playlist,
    ),
];

/// The `bitrate` NetEase is asked for at the level the app asks for: 128
/// kbit/s (its lowest) for Low and Normal, 320 for High, 999 for lossless.
pub fn bitrate(quality: Quality) -> u32 {
    match quality {
        Quality::Low | Quality::Normal => 128,
        Quality::High => 320,
        Quality::Lossless => 999,
    }
}

fn offset(page: &PageRequest) -> Result<u32> {
    page.cursor.as_deref().map_or(Ok(0), |c| {
        c.parse()
            .map_err(|_| PluginError::invalid_input("not a page cursor"))
    })
}

/// The error for an answer with `what` that Jellyfesh cannot read, in plain
/// words; the answer's shape (its keys and their types, never the values)
/// goes to the log.
fn unreadable(what: &str, data: &Value) -> PluginError {
    jf_plugin_sdk::warn!("cannot read NetEase's answer with {what}: {}", items::shape(data));
    PluginError::service(format!(
        "NetEase Cloud Music sent {what} in a form Jellyfesh cannot read."
    ))
}

/// "your created playlists" for a list's title.
fn yours(title: &str) -> String {
    format!("your {}", title.to_lowercase())
}

/// A page from a list answer: `next` when NetEase says there is more
/// (`hasMore`, or a count beyond this page), or, when it does not say, when
/// the page came back full (a page longer than asked for is the whole
/// list). `None`, never an empty page, when the answer has no list
/// Jellyfesh can read, an empty one where NetEase counts entries from
/// `offset` on, or entries none of which Jellyfesh can read.
fn read_page(
    data: &Value,
    offset: u32,
    limit: u32,
    item: impl Fn(&Value) -> Option<Item>,
) -> Option<ItemPage> {
    let count = items::count(data);
    let list = items::listing(data)
        .filter(|list| !list.is_empty() || count.is_none_or(|n| n <= u64::from(offset)))?;
    let found: Vec<Item> = list.iter().filter_map(item).collect();
    if found.is_empty() && !list.is_empty() {
        return None;
    }
    let len = u32::try_from(list.len()).unwrap_or(u32::MAX);
    let end = offset.saturating_add(len);
    let more = len <= limit
        && items::has_more(data).unwrap_or_else(|| count.map_or(len == limit, |n| u64::from(end) < n));
    Some(ItemPage {
        items: found,
        next: (more && len > 0).then(|| end.to_string()),
        total: count.and_then(|n| u32::try_from(n).ok()),
    })
}

/// [`read_page`] of an answer with `what`, or an error in plain words.
fn page_of(
    data: &Value,
    offset: u32,
    limit: u32,
    what: &str,
    item: impl Fn(&Value) -> Option<Item>,
) -> Result<ItemPage> {
    read_page(data, offset, limit, item).ok_or_else(|| unreadable(what, data))
}

/// The account's playlists for the library (`library.playlists`): the
/// created ones (the person's own), then the collected ones (followed),
/// each list paged by NetEase. The cursor names the list and the offset in
/// it (`created:20`, `collected:0`).
fn library_playlists(page: &PageRequest) -> Result<PlaylistPage> {
    let limit = page.limit.clamp(1, PAGE_MAX);
    let (mut list, mut offset) = match page.cursor.as_deref() {
        None | Some("") => (0, 0),
        Some(cursor) => {
            let (name, at) = cursor
                .split_once(':')
                .ok_or_else(|| PluginError::invalid_input("not a page cursor"))?;
            let list = LISTS
                .iter()
                .position(|(id, _, _)| *id == name)
                .ok_or_else(|| PluginError::invalid_input("not a page cursor"))?;
            let offset = at
                .parse()
                .map_err(|_| PluginError::invalid_input("not a page cursor"))?;
            (list, offset)
        }
    };
    let mut entries = Vec::new();
    loop {
        let (id, title, path) = LISTS[list];
        let want = limit - entries.len() as u32;
        let data = api::call(path, &json!({ "limit": want, "offset": offset }))?;
        let found = page_of(&data, offset, want, &yours(title), items::playlist)?;
        let owned = id == "created";
        entries.extend(found.items.into_iter().map(|item| PlaylistEntry {
            item,
            owned,
            songs_listed: true,
        }));
        if let Some(next) = found.next {
            return Ok(PlaylistPage::with_next(entries, format!("{id}:{next}")));
        }
        if list + 1 == LISTS.len() {
            return Ok(PlaylistPage::new(entries));
        }
        list += 1;
        offset = 0;
        if entries.len() as u32 >= limit {
            return Ok(PlaylistPage::with_next(entries, format!("{}:0", LISTS[list].0)));
        }
    }
}

/// The liked songs' playlist (NetEase's red-heart playlist, which every
/// account has): its id, and how many songs NetEase says it holds. An error
/// in plain words when the answer names no playlist.
fn liked_playlist() -> Result<(String, Option<u64>)> {
    let data = api::call(STAR, &json!({}))?;
    items::liked(&data).ok_or_else(|| {
        jf_plugin_sdk::warn!(
            "NetEase's liked-songs answer names no playlist: {}",
            items::shape(&data)
        );
        PluginError::service("NetEase Cloud Music didn't say which playlist holds your liked songs.")
    })
}

/// A page of a playlist's songs; an error, not an empty page, when NetEase
/// said the playlist holds songs from `offset` on (`said`) but sent none.
fn songs(playlist: &str, offset: u32, limit: u32, what: &str, said: Option<u64>) -> Result<ItemPage> {
    let data = api::call(
        SONGS,
        &json!({ "playlistId": playlist, "limit": limit, "offset": offset }),
    )?;
    let page = page_of(&data, offset, limit, what, items::song)?;
    if page.items.is_empty() && said.is_some_and(|n| n > u64::from(offset)) {
        return Err(unreadable(what, &data));
    }
    Ok(page)
}

/// A page of the liked songs (newest like first, as NetEase keeps them).
fn liked_songs(offset: u32, limit: u32) -> Result<ItemPage> {
    let (playlist, said) = liked_playlist()?;
    songs(&playlist, offset, limit, "your liked songs", said)
}

/// Items for a shelf, or none when this list failed (the page goes on
/// without it); the sign-in, the rate limit and the network fail the page.
fn optional(result: Result<Vec<Item>>, what: &str) -> Result<Vec<Item>> {
    match result {
        Ok(items) => Ok(items),
        Err(error @ (PluginError::AuthRequired | PluginError::RateLimited | PluginError::Network(_))) => {
            Err(error)
        }
        Err(error) => {
            jf_plugin_sdk::warn!("no {what} for Discover: {error:?}");
            Ok(Vec::new())
        }
    }
}

/// The Discover page: the liked songs, then the created and collected
/// playlists, as the open platform lists them.
fn discover_shelves() -> Result<Vec<Shelf>> {
    let mut shelves = Vec::new();
    // Liked songs that cannot be read leave their shelf out (the error is
    // logged, and shown where they are listed: Favourites, the folder).
    let liked = optional(liked_songs(0, SHELF).map(|page| page.items), LIKED.1)?;
    if !liked.is_empty() {
        shelves.push(Shelf {
            id: "liked".into(),
            title: LIKED.1.into(),
            style: ShelfStyle::Row,
            items: liked,
            more: Some(LIKED.0.into()),
        });
    }
    for (id, title, path) in LISTS {
        let listed = api::call(path, &json!({ "limit": SHELF, "offset": 0 }))
            .and_then(|data| page_of(&data, 0, SHELF, &yours(title), items::playlist))
            .map(|page| page.items);
        let listed = optional(listed, title)?;
        if !listed.is_empty() {
            shelves.push(Shelf {
                id: id.into(),
                title: title.into(),
                style: ShelfStyle::Grid,
                items: listed,
                more: Some(id.into()),
            });
        }
    }
    Ok(shelves)
}

/// Search results. The core asks for one kind at a time; when several are
/// asked (none means every kind), they share the page and each goes on from
/// the same offset. The cursor is the offset.
fn search(query: &str, filters: &SearchFilters, page: &PageRequest) -> Result<ItemPage> {
    let keyword = query.trim();
    let asked: Vec<_> = SEARCHES
        .iter()
        .filter(|(kind, ..)| filters.kinds.is_empty() || filters.kinds.contains(kind))
        .collect();
    if keyword.is_empty() || asked.is_empty() {
        return Ok(ItemPage::new(Vec::new()));
    }
    let offset = offset(page)?;
    let limit = (page.limit.clamp(1, PAGE_MAX) / asked.len() as u32).max(1);
    let mut pages = Vec::new();
    for (_, path, what, read) in asked {
        let data = api::call(
            path,
            &json!({ "keyword": keyword, "limit": limit, "offset": offset }),
        )?;
        pages.push(page_of(&data, offset, limit, what, read)?);
    }
    if pages.len() == 1 {
        return Ok(pages.remove(0));
    }
    let more = pages.iter().any(|p| p.next.is_some());
    Ok(ItemPage {
        items: pages.into_iter().flat_map(|p| p.items).collect(),
        next: more.then(|| offset.saturating_add(limit).to_string()),
        total: None,
    })
}

/// Codec hint from the stream URL's file extension.
fn codec(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next().unwrap_or_default();
    match path.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "mp3" => Some("mp3".into()),
        "flac" => Some("flac".into()),
        "m4a" | "mp4" | "aac" => Some("aac".into()),
        _ => None,
    }
}

impl Plugin for NetEase {
    fn capabilities() -> Vec<Capability> {
        vec![
            Capability::Auth,
            Capability::Search,
            Capability::Browse,
            Capability::ResolveStream,
        ]
    }

    fn settings_changed(settings: &Value) -> Result<()> {
        // A `quality` left from version 0.0.7 is ignored: the app's level
        // decides (the host drops it when the plugin updates).
        api::set_server(
            settings
                .get(TEST_SERVER)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty()),
        );
        Ok(())
    }

    fn search(query: String, filters: SearchFilters, page: PageRequest) -> Result<ItemPage> {
        search(&query, &filters, &page)
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
        // No revocation is documented; the host deletes the tokens, the App ID
        // and the private key from the keystore.
        Ok(())
    }

    fn browse(node: Option<Item>, page: PageRequest) -> Result<ItemPage> {
        let Some(node) = node else {
            return Ok(ItemPage::new(
                LISTS
                    .iter()
                    .map(|(id, title, _)| (*id, *title))
                    .chain([LIKED])
                    .map(|(id, title)| Item::new(ItemKind::Folder, id, title))
                    .collect(),
            ));
        };
        let offset = offset(&page)?;
        let limit = page.limit.clamp(1, PAGE_MAX);
        match node.kind {
            ItemKind::Folder if node.id == LIKED.0 => liked_songs(offset, limit),
            ItemKind::Folder => {
                let (_, title, path) = LISTS
                    .iter()
                    .find(|(id, _, _)| *id == node.id)
                    .ok_or(PluginError::NotFound)?;
                let data = api::call(path, &json!({ "limit": limit, "offset": offset }))?;
                page_of(&data, offset, limit, &yours(title), items::playlist)
            }
            ItemKind::Playlist => songs(&node.id, offset, limit, "this playlist's songs", None),
            ItemKind::Album => {
                let album = node.id.strip_prefix(items::ALBUM).unwrap_or(&node.id);
                let data = api::call(
                    ALBUM_SONGS,
                    &json!({ "albumId": album, "limit": limit, "offset": offset }),
                )?;
                page_of(&data, offset, limit, "this album's songs", items::song)
            }
            // The popular songs are one list: whether NetEase pages it is
            // not known.
            ItemKind::Artist if offset == 0 => {
                let artist = node.id.strip_prefix(items::ARTIST).unwrap_or(&node.id);
                let data = api::call(ARTIST_SONGS, &json!({ "artistId": artist }))?;
                let page = page_of(&data, 0, limit, "this artist's popular songs", items::song)?;
                Ok(ItemPage { next: None, ..page })
            }
            ItemKind::Artist => Ok(ItemPage::new(Vec::new())),
            _ => Err(PluginError::NotFound),
        }
    }

    fn discover() -> Result<Vec<Shelf>> {
        discover_shelves()
    }

    /// The liked songs (the red-heart playlist, newest like first, as
    /// NetEase keeps it). The open platform has no liked albums or artists.
    fn library_likes(kind: ItemKind, page: PageRequest) -> Result<ItemPage> {
        if kind != ItemKind::Track {
            return Err(PluginError::Unsupported);
        }
        liked_songs(offset(&page)?, page.limit.clamp(1, PAGE_MAX))
    }

    fn library_playlists(page: PageRequest) -> Result<PlaylistPage> {
        library_playlists(&page)
    }

    fn details(item: Item) -> Result<ItemDetails> {
        if item.kind != ItemKind::Playlist {
            // The open platform's song detail is for playing; nothing else
            // about songs, albums or artists is known.
            return Err(PluginError::Unsupported);
        }
        let data = api::call(PLAYLIST, &json!({ "playlistId": item.id }))?;
        items::one(&data)
            .map(items::playlist_details)
            .ok_or(PluginError::NotFound)
    }

    fn resolve_stream(item: Item, quality: Quality) -> Result<StreamInfo> {
        if item.kind != ItemKind::Track || item.id.is_empty() {
            return Err(PluginError::invalid_input("not a NetEase song"));
        }
        let bitrate = bitrate(quality);
        let data = api::call(
            DETAIL,
            &json!({ "songId": item.id, "withUrl": true, "bitrate": bitrate }),
        )?;
        // The detail's own flags win over the list's.
        let access = items::access(&data);
        if let Some(refused) = items::refusal(access) {
            return Err(PluginError::service(refused.message.unwrap_or_default()));
        }
        // A free trial window without a free full listen is a preview: never
        // stream it, whatever the URL would play.
        let trial = data
            .get("freeTrail")
            .is_some_and(|t| t.get("end").and_then(Value::as_f64).is_some_and(|end| end > 0.0));
        if trial && access != Access::Full {
            return Err(PluginError::service(
                items::refusal(Access::Preview)
                    .and_then(|r| r.message)
                    .unwrap_or_default(),
            ));
        }
        let url = data
            .get("playUrl")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .ok_or_else(|| {
                PluginError::service(
                    "NetEase Cloud Music has no audio for this song, or no permission to play it \
                     (该歌曲暂无音源或暂无播放权限).",
                )
            })?;
        let url = items::secure(url);
        let mut info = StreamInfo::new(url.clone());
        info.codec = codec(&url);
        info.bitrate_kbps = data
            .get("br")
            .and_then(Value::as_u64)
            .map(|br| u32::try_from(br.div_ceil(1000)).unwrap_or(u32::MAX));
        Ok(info)
    }
}

jf_plugin_sdk::export_plugin!(NetEase);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualities_map_to_netease_bitrates() {
        assert_eq!(bitrate(Quality::Low), 128);
        assert_eq!(bitrate(Quality::Normal), 128);
        assert_eq!(bitrate(Quality::High), 320);
        assert_eq!(bitrate(Quality::Lossless), 999);
    }

    #[test]
    fn codecs_come_from_the_extension() {
        assert_eq!(
            codec("https://m7.music.126.net/a/b.mp3?vuutv=x").as_deref(),
            Some("mp3")
        );
        assert_eq!(
            codec("https://m7.music.126.net/a/b.FLAC").as_deref(),
            Some("flac")
        );
        assert_eq!(codec("https://m7.music.126.net/a/b"), None);
    }

    fn any(_: &Value) -> Option<Item> {
        Some(Item::new(ItemKind::Track, "x", "y"))
    }

    fn next(data: Value, offset: u32, limit: u32) -> Option<String> {
        read_page(&data, offset, limit, any).unwrap().next
    }

    #[test]
    fn pages_continue_when_netease_says_so_or_the_page_is_full() {
        assert_eq!(next(json!({ "records": [{}, {}] }), 4, 2).as_deref(), Some("6"));
        assert_eq!(next(json!([{}]), 0, 2), None);
        let said = json!({ "list": [{}], "hasMore": true, "total": 9 });
        let page = read_page(&said, 0, 50, any).unwrap();
        assert_eq!((page.next.as_deref(), page.total), (Some("1"), Some(9)));
        // A count says whether a full page is the last one.
        assert_eq!(next(json!({ "records": [{}, {}], "recordCount": 4 }), 2, 2), None);
        assert_eq!(
            next(json!({ "records": [{}, {}], "recordCount": 25 }), 20, 2).as_deref(),
            Some("22")
        );
        assert_eq!(
            next(json!({ "records": [{}], "recordCount": 25 }), 0, 20).as_deref(),
            Some("1")
        );
        // More entries than asked for: NetEase sent the whole list.
        assert_eq!(next(json!([{}, {}, {}]), 0, 2), None);
        // Nothing to read is no page at all, not a last one.
        assert_eq!(next(json!({ "records": [], "recordCount": 0 }), 0, 20), None);
        assert_eq!(next(json!({ "records": [], "recordCount": 5 }), 5, 20), None);
    }

    /// The plain words and the log (the plugin host's) are checked in
    /// tests/netease.rs.
    #[test]
    fn answers_that_cannot_be_read_are_no_page() {
        for unreadable in [
            json!({ "result": { "songs": [{}] } }),
            json!({ "recordCount": 3 }),
            json!({ "records": [], "recordCount": 6 }),
            json!("text"),
        ] {
            assert!(read_page(&unreadable, 0, 20, any).is_none(), "{unreadable}");
        }
        // Entries none of which read; some that do not read are left out.
        let records = json!({ "records": [{ "name": "a" }, {}] });
        assert!(read_page(&records, 0, 20, |_| None).is_none());
        let named = |v: &Value| v.get("name").and_then(|_| any(v));
        assert_eq!(read_page(&records, 0, 20, named).unwrap().items.len(), 1);
    }
}
