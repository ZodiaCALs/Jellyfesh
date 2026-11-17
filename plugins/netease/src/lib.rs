//! NetEase Cloud Music for Jellyfesh: the open platform with the person's own
//! individual-developer credentials (App ID and RSA private key) and a QR
//! sign-in with the NetEase Cloud Music app. The account's playlists
//! (created, collected, starred), their songs, and streams by quality, with
//! NetEase's rules on what each account may play: VIP songs and digital
//! albums are paywalled, previews are not played.
//!
//! Search, liked songs, recommendations and lyrics wait for NetEase's API
//! documentation: their paths were not in what the research could read.
//! Tested only against a mock server written from the research, not yet
//! against the live service.

mod api;
mod items;
mod login;

use std::cell::RefCell;

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

thread_local! {
    /// The `quality` setting: `app` or a fixed quality.
    static QUALITY: RefCell<String> = RefCell::new("app".to_owned());
}

/// The three playlist lists of the account, by folder id.
const LISTS: [(&str, &str, &str); 3] = [
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
    (
        "starred",
        "Starred Playlists",
        "/openapi/music/basic/playlist/star/get/v2",
    ),
];
const SONGS: &str = "/openapi/music/basic/playlist/song/list/get/v3";
const DETAIL: &str = "/openapi/music/basic/song/detail/get/v2";

/// The `bitrate` NetEase is asked for: 128 and 320 kbit/s, 999 for lossless.
pub fn bitrate(quality: Quality, setting: &str) -> u32 {
    let quality = match setting {
        "low" => Quality::Low,
        "normal" => Quality::Normal,
        "high" => Quality::High,
        "lossless" => Quality::Lossless,
        _ => quality,
    };
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
        total: ["total", "count"]
            .iter()
            .find_map(|k| data.get(*k).and_then(Value::as_u64))
            .and_then(|n| u32::try_from(n).ok()),
    }
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
        api::set_server(
            settings
                .get(TEST_SERVER)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty()),
        );
        let quality = settings
            .get("quality")
            .and_then(Value::as_str)
            .unwrap_or("app")
            .to_owned();
        QUALITY.with(|q| *q.borrow_mut() = quality);
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
                    .map(|(id, title, _)| Item::new(ItemKind::Folder, *id, *title))
                    .collect(),
            ));
        };
        let offset = offset(&page)?;
        let limit = page.limit.clamp(1, PAGE_MAX);
        match node.kind {
            ItemKind::Folder => {
                let (_, _, path) = LISTS
                    .iter()
                    .find(|(id, _, _)| *id == node.id)
                    .ok_or(PluginError::NotFound)?;
                let data = api::call(path, &json!({ "limit": limit, "offset": offset }))?;
                Ok(page_of(&data, offset, limit, items::playlist))
            }
            ItemKind::Playlist => {
                let data = api::call(
                    SONGS,
                    &json!({ "playlistId": node.id, "limit": limit, "offset": offset }),
                )?;
                Ok(page_of(&data, offset, limit, items::song))
            }
            _ => Err(PluginError::NotFound),
        }
    }

    fn resolve_stream(item: Item, quality: Quality) -> Result<StreamInfo> {
        if item.kind != ItemKind::Track || item.id.is_empty() {
            return Err(PluginError::invalid_input("not a NetEase song"));
        }
        let bitrate = QUALITY.with(|q| bitrate(quality, &q.borrow()));
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
        assert_eq!(bitrate(Quality::Low, "app"), 128);
        assert_eq!(bitrate(Quality::Normal, "app"), 128);
        assert_eq!(bitrate(Quality::High, "app"), 320);
        assert_eq!(bitrate(Quality::Lossless, "app"), 999);
        assert_eq!(bitrate(Quality::Lossless, "high"), 320, "a fixed quality wins");
        assert_eq!(bitrate(Quality::Low, "lossless"), 999);
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
