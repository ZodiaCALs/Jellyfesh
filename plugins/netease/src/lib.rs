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
//! Search, recommendations, comments and lyrics wait for NetEase's API
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
/// Items on a Discover shelf.
const SHELF: u32 = 20;

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

/// A page from a list answer: `next` when NetEase says there is more, or
/// (when it does not say) when the page came back full.
fn page_of(data: &Value, offset: u32, limit: u32, item: impl Fn(&Value) -> Option<Item>) -> ItemPage {
    let list = items::list(data);
    let more = items::has_more(data).unwrap_or(list.len() as u32 >= limit);
    ItemPage {
        items: list.iter().filter_map(item).collect(),
        next: (more && !list.is_empty()).then(|| (offset + list.len() as u32).to_string()),
        total: ["recordCount", "total", "count"]
            .iter()
            .find_map(|k| data.get(*k).and_then(Value::as_u64))
            .and_then(|n| u32::try_from(n).ok()),
    }
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
        let (id, _, path) = LISTS[list];
        let want = limit - entries.len() as u32;
        let data = api::call(path, &json!({ "limit": want, "offset": offset }))?;
        let found = page_of(&data, offset, want, items::playlist);
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

/// The id of the liked songs' playlist, if the account has one.
fn liked_playlist() -> Result<Option<String>> {
    let data = api::call(STAR, &json!({}))?;
    Ok(items::one(&data).and_then(items::playlist).map(|p| p.id))
}

/// A page of a playlist's songs.
fn songs(playlist: &str, offset: u32, limit: u32) -> Result<ItemPage> {
    let data = api::call(
        SONGS,
        &json!({ "playlistId": playlist, "limit": limit, "offset": offset }),
    )?;
    Ok(page_of(&data, offset, limit, items::song))
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
    let liked = liked_playlist().and_then(|id| match id {
        Some(id) => songs(&id, 0, SHELF).map(|page| page.items),
        None => Ok(Vec::new()),
    });
    let liked = optional(liked, LIKED.1)?;
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
            .map(|data| page_of(&data, 0, SHELF, items::playlist).items);
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
        vec![Capability::Auth, Capability::Browse, Capability::ResolveStream]
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
            ItemKind::Folder if node.id == LIKED.0 => match liked_playlist()? {
                Some(playlist) => songs(&playlist, offset, limit),
                None => Ok(ItemPage::new(Vec::new())),
            },
            ItemKind::Folder => {
                let (_, _, path) = LISTS
                    .iter()
                    .find(|(id, _, _)| *id == node.id)
                    .ok_or(PluginError::NotFound)?;
                let data = api::call(path, &json!({ "limit": limit, "offset": offset }))?;
                Ok(page_of(&data, offset, limit, items::playlist))
            }
            ItemKind::Playlist => songs(&node.id, offset, limit),
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
        let offset = offset(&page)?;
        match liked_playlist()? {
            Some(playlist) => songs(&playlist, offset, page.limit.clamp(1, PAGE_MAX)),
            None => Ok(ItemPage::new(Vec::new())),
        }
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

    #[test]
    fn pages_continue_when_netease_says_so_or_the_page_is_full() {
        let full = json!({ "records": [{}, {}] });
        assert_eq!(page_of(&full, 4, 2, |_| None).next.as_deref(), Some("6"));
        let short = json!([{}]);
        assert_eq!(page_of(&short, 0, 2, |_| None).next, None);
        let said = json!({ "list": [{}], "hasMore": true, "total": 9 });
        let page = page_of(&said, 0, 50, |_| None);
        assert_eq!((page.next.as_deref(), page.total), (Some("1"), Some(9)));
    }
}
