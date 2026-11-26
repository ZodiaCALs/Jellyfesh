//! A thin test plugin for `jf-core`'s source tests: search, browse, stream
//! URLs, lyrics and artwork all come from the test server named by the
//! `server_url` setting, so each test decides what the "service" answers
//! (and can count what the core asked for).
//!
//! | Export | Request | Answer |
//! | --- | --- | --- |
//! | `search` | `GET /search?q=&limit=` | `{"items": [hit], "next": cursor?}` |
//! | `browse` | `GET /browse?node=&kind=&cursor=&limit=` | the same |
//! | `resolve-stream` | `GET /resolve?id=` | `{"url", "headers": [[name, value]], "expires_at_ms"?}` |
//! | `lyrics` | `GET /lyrics?id=&title=` | `{"format": "lrc" or "plain", "text"}`, 404 for none |
//! | `artwork` | `GET /artwork?id=&size=` | `{"url", "headers": [[name, value]]}`, 404 for none |
//!
//! A hit is `{"id", "kind", "title", "artist"?, "album"?, "duration_ms"?,
//! "isrc"?, "artwork"?, "playable"?, "reason"?}` (`kind` is `track`,
//! `album`, `artist`, `playlist` or `folder`; `reason` is `paywall` or
//! `region`).

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;
use serde::Deserialize;

struct Catalog;

thread_local! {
    static SERVER: RefCell<String> = const { RefCell::new(String::new()) };
}

fn server() -> Result<String> {
    let url = SERVER.with(|s| s.borrow().clone());
    if url.is_empty() {
        Err(PluginError::invalid_input("no server configured"))
    } else {
        Ok(url.trim_end_matches('/').to_string())
    }
}

#[derive(Deserialize)]
struct Hit {
    id: String,
    kind: String,
    title: String,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<u32>,
    isrc: Option<String>,
    artwork: Option<String>,
    playable: Option<bool>,
    reason: Option<String>,
}

#[derive(Deserialize)]
struct Page {
    items: Vec<Hit>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct Link {
    url: String,
    #[serde(default)]
    headers: Vec<(String, String)>,
    expires_at_ms: Option<u64>,
    #[serde(default)]
    codec: Option<String>,
    #[serde(default)]
    bitrate_kbps: Option<u32>,
}

#[derive(Deserialize)]
struct Lyrics {
    format: String,
    text: String,
}

fn kind_name(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Track => "track",
        ItemKind::Album => "album",
        ItemKind::Artist => "artist",
        ItemKind::Playlist => "playlist",
        ItemKind::Folder => "folder",
    }
}

fn item(hit: Hit) -> Item {
    let kind = match hit.kind.as_str() {
        "album" => ItemKind::Album,
        "artist" => ItemKind::Artist,
        "playlist" => ItemKind::Playlist,
        "folder" => ItemKind::Folder,
        _ => ItemKind::Track,
    };
    let mut item = Item::new(kind, hit.id, hit.title);
    item.artist = hit.artist;
    item.album = hit.album;
    item.duration_ms = hit.duration_ms;
    item.isrc = hit.isrc;
    if let Some(url) = hit.artwork {
        item = item.artwork(url);
    }
    if hit.playable == Some(false) {
        let reason = match hit.reason.as_deref() {
            Some("paywall") => UnplayableReason::Paywall,
            Some("region") => UnplayableReason::Region,
            _ => UnplayableReason::Unavailable,
        };
        item = item.unplayable(reason, None);
    }
    item
}

fn page(answer: Page) -> ItemPage {
    let items = answer.items.into_iter().map(item).collect();
    match answer.next {
        Some(next) => ItemPage::with_next(items, next),
        None => ItemPage::new(items),
    }
}

impl Plugin for Catalog {
    fn capabilities() -> Vec<Capability> {
        vec![
            Capability::Search,
            Capability::Browse,
            Capability::ResolveStream,
            Capability::Lyrics,
            Capability::Artwork,
        ]
    }

    fn settings_changed(settings: &serde_json::Value) -> Result<()> {
        let url = settings["server_url"].as_str().unwrap_or_default().to_string();
        SERVER.with(|s| *s.borrow_mut() = url);
        Ok(())
    }

    fn search(query: String, _filters: SearchFilters, request: PageRequest) -> Result<ItemPage> {
        let answer: Page = http::Request::get(format!("{}/search", server()?))
            .query(&[("q", &query), ("limit", &request.limit.to_string())])
            .send_json()?;
        Ok(page(answer))
    }

    fn browse(node: Option<Item>, request: PageRequest) -> Result<ItemPage> {
        let (id, kind) = match &node {
            Some(node) => (node.id.as_str(), kind_name(node.kind)),
            None => ("", ""),
        };
        let cursor = request.cursor.unwrap_or_default();
        let answer: Page = http::Request::get(format!("{}/browse", server()?))
            .query(&[
                ("node", id),
                ("kind", kind),
                ("cursor", &cursor),
                ("limit", &request.limit.to_string()),
            ])
            .send_json()?;
        Ok(page(answer))
    }

    fn resolve_stream(item: Item, quality: Quality) -> Result<StreamInfo> {
        let level = match quality {
            Quality::Lossless => "lossless",
            Quality::High => "high",
            Quality::Normal => "normal",
            Quality::Low => "low",
        };
        let link: Link = http::Request::get(format!("{}/resolve", server()?))
            .query(&[("id", item.id.as_str()), ("quality", level)])
            .send_json()?;
        let mut info = StreamInfo::new(link.url);
        info.headers = link
            .headers
            .into_iter()
            .map(|(name, value)| Header::new(name, value))
            .collect();
        info.expires_at_ms = link.expires_at_ms;
        info.codec = link.codec;
        info.bitrate_kbps = link.bitrate_kbps;
        Ok(info)
    }

    fn lyrics(item: Item) -> Result<Option<LyricPayload>> {
        let response = http::Request::get(format!("{}/lyrics", server()?))
            .query(&[("id", &item.id), ("title", &item.title)])
            .send()?;
        if response.status == 404 {
            return Ok(None);
        }
        let lyrics: Lyrics = response.error_for_status()?.json()?;
        let format = if lyrics.format == "plain" {
            LyricFormat::Plain
        } else {
            LyricFormat::Lrc
        };
        Ok(Some(LyricPayload::new(format, lyrics.text)))
    }

    fn artwork(item: Item, size: u32) -> Result<Option<ImageRef>> {
        let response = http::Request::get(format!("{}/artwork", server()?))
            .query(&[("id", &item.id), ("size", &size.to_string())])
            .send()?;
        if response.status == 404 {
            return Ok(None);
        }
        let link: Link = response.error_for_status()?.json()?;
        let mut image = ImageRef::new(link.url);
        image.headers = link
            .headers
            .into_iter()
            .map(|(name, value)| Header::new(name, value))
            .collect();
        Ok(Some(image))
    }
}

jf_plugin_sdk::export_plugin!(Catalog);
