//! QQ Music for Jellyfesh: QQ Music's official open API (`qqmusic-skills`
//! 0.0.3) with the person's own API key. Search, browse (Daily 30, Guess You
//! Like, AI playlists, charts, playlists) and personal recommendations, as
//! metadata and links: the API has no audio, so nothing plays here and every
//! item opens in QQ Music (ADR-0018).
//!
//! Tested only against a mock server written from the API documentation,
//! not yet against the live service.

mod api;
mod items;
mod pager;

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};

use api::{Failure, KEY_SECRET};
use items::{Album, ChartGroup, Playlist, Singer, Song, list};
use pager::{Cursor, Fill, ServerPage, cached};

struct QqMusic;

/// The setting only test packages have (jf-plugin-testkit's
/// `with_test_server_setting`): the address of a mock server to use instead
/// of QQ Music. Published packages' schemas do not name it, so the host never
/// passes it to them.
const TEST_SERVER: &str = "test_server";

thread_local! {
    static BASE: RefCell<String> = RefCell::new(api::BASE.to_owned());
}

fn base() -> String {
    BASE.with(|b| b.borrow().clone())
}

/// Calls the API with the stored key.
fn call<T: for<'de> Deserialize<'de>>(path: &str, params: Value) -> Result<T> {
    let key = kv::secret::get(KEY_SECRET)?.ok_or(PluginError::AuthRequired)?;
    Ok(api::call_with(&base(), &key, path, params)?)
}

/// The search types QQ Music offers that Jellyfesh shows, in the order a
/// search of every kind lists them.
const SEARCH_TYPES: [(&str, ItemKind); 4] = [
    ("0", ItemKind::Track),
    ("5", ItemKind::Artist),
    ("1", ItemKind::Album),
    ("2", ItemKind::Playlist),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchAnswer {
    #[serde(default)]
    songs: Option<Vec<Song>>,
    #[serde(default)]
    albums: Option<Vec<Album>>,
    #[serde(default)]
    playlists: Option<Vec<Playlist>>,
    #[serde(default)]
    singers: Option<Vec<Singer>>,
    #[serde(default)]
    has_more: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SongList {
    #[serde(default)]
    songlist: Option<Vec<Song>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Tracks {
    #[serde(default)]
    track_list: Option<Vec<Song>>,
    #[serde(default)]
    has_more: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Playlists {
    #[serde(default)]
    playlists: Option<Vec<Playlist>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Charts {
    #[serde(default)]
    group_list: Option<Vec<ChartGroup>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Version {
    #[serde(default)]
    upgrade_info: Option<Upgrade>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Upgrade {
    #[serde(default)]
    need_upgrade: bool,
    #[serde(default)]
    latest_version: Option<String>,
}

fn songs(list_of: Option<Vec<Song>>) -> Vec<Item> {
    list(list_of).into_iter().filter_map(Song::item).collect()
}

/// One server page of search type `stream` (an index into SEARCH_TYPES).
fn search_page(query: &str, stream: u8, page: u32) -> Result<ServerPage> {
    let (code, kind) = SEARCH_TYPES[usize::from(stream)];
    cached(format!("search|{code}|{page}|{query}"), || {
        let answer: SearchAnswer = call(
            "/discover/search",
            json!({ "keyword": query, "type": code, "page": page }),
        )?;
        let items: Vec<Item> = match kind {
            ItemKind::Track => songs(answer.songs),
            ItemKind::Album => list(answer.albums).into_iter().filter_map(Album::item).collect(),
            ItemKind::Playlist => list(answer.playlists)
                .into_iter()
                .filter_map(Playlist::item)
                .collect(),
            _ => list(answer.singers)
                .into_iter()
                .filter_map(Singer::item)
                .collect(),
        };
        Ok((items, answer.has_more))
    })
}

/// The cache key prefix of a playlist's or chart's pages.
fn tracks_key(path: &str, params: &Value) -> String {
    format!("{path}|{params}|")
}

/// One server page of a playlist's or chart's tracks.
fn tracks_page(path: &str, params: Value, page: u32) -> Result<ServerPage> {
    cached(format!("{}{page}", tracks_key(path, &params)), || {
        let mut params = params;
        params["page"] = json!(page);
        let answer: Tracks = call(path, params)?;
        Ok((songs(answer.track_list), answer.has_more))
    })
}

/// Where a page starts: the host's cursor, or the first page of `streams`,
/// read afresh (the cached pages under `key` are dropped).
fn cursor(page: &PageRequest, streams: &[u8], key: &str) -> Result<Cursor> {
    match page.cursor.as_deref() {
        Some(text) => Cursor::decode(text, SEARCH_TYPES.len() as u8),
        None => {
            pager::invalidate(key);
            Ok(Cursor::start(streams))
        }
    }
}

fn folder(id: &str, title: &str) -> Item {
    Item::new(ItemKind::Folder, id, title)
}

fn charts() -> Result<Vec<ChartGroup>> {
    let answer: Charts = call("/charts", json!({}))?;
    Ok(list(answer.group_list))
}

impl Plugin for QqMusic {
    fn capabilities() -> Vec<Capability> {
        vec![
            Capability::Auth,
            Capability::Search,
            Capability::Browse,
            Capability::Recommendations,
        ]
    }

    fn settings_changed(settings: &Value) -> Result<()> {
        let base = settings
            .get(TEST_SERVER)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or(api::BASE)
            .to_owned();
        BASE.with(|b| *b.borrow_mut() = base);
        pager::forget();
        Ok(())
    }

    fn auth_begin(flow: Flow) -> Result<FlowStep> {
        if flow.kind != AuthKind::ApiKey {
            return Err(PluginError::Unsupported);
        }
        Ok(FlowStep::new(Step::AskFields(vec![FieldSpec {
            placeholder: Some("qmk-…".into()),
            help: Some(
                "Your own key from y.qq.com/n/ryqq_v2/qqmusic_skills, for your personal use only. \
                 Jellyfesh keeps it in your system's keystore and sends it only to QQ Music."
                    .into(),
            ),
            ..FieldSpec::secret("api-key", "API Key")
        }])))
    }

    fn auth_submit(_flow: Flow, fields: Vec<FieldValue>) -> Result<FlowStep> {
        let key = fields
            .iter()
            .find(|f| f.name == "api-key")
            .map(|f| f.value.trim().to_owned())
            .unwrap_or_default();
        if !api::plausible_key(&key) {
            return Ok(FlowStep::failed(
                "A QQ Music API key starts with qmk- and has no spaces. Copy it again from \
                 y.qq.com/n/ryqq_v2/qqmusic_skills.",
            ));
        }
        // A cheap call that needs the key.
        match api::call_with::<Version>(&base(), &key, "/skill/version", json!({})) {
            Ok(version) => {
                if let Some(upgrade) = version.upgrade_info.filter(|u| u.need_upgrade) {
                    jf_plugin_sdk::warn!(
                        "QQ Music reports skill version {} as current; this plugin speaks {}",
                        upgrade.latest_version.unwrap_or_default(),
                        api::SKILL_VERSION
                    );
                }
            }
            Err(Failure::Key) => {
                return Ok(FlowStep::failed(
                    "QQ Music did not accept this API key. Check it on the key page, or reset it there \
                     and paste the new one.",
                ));
            }
            Err(Failure::Network(why)) => {
                return Ok(FlowStep::failed(format!("Cannot reach QQ Music: {why}")));
            }
            Err(Failure::RateLimited) => {
                return Ok(FlowStep::failed(
                    "QQ Music asks to slow down. Try again in a minute.",
                ));
            }
            Err(Failure::Service(why)) => return Ok(FlowStep::failed(format!("QQ Music answered: {why}"))),
        }
        kv::secret::set(KEY_SECRET, &key)?;
        pager::forget();
        let tail: String = key
            .chars()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        Ok(FlowStep::done(format!("API key …{tail}")))
    }

    fn logout() -> Result<()> {
        // The key lives on QQ Music's key page; the host deletes the stored copy.
        pager::forget();
        Ok(())
    }

    fn search(query: String, filters: SearchFilters, page: PageRequest) -> Result<ItemPage> {
        let query = query.trim().to_owned();
        if query.is_empty() {
            return Ok(ItemPage::new(Vec::new()));
        }
        let streams: Vec<u8> = (0..SEARCH_TYPES.len() as u8)
            .filter(|s| filters.kinds.is_empty() || filters.kinds.contains(&SEARCH_TYPES[usize::from(*s)].1))
            .collect();
        if streams.is_empty() {
            return Ok(ItemPage::new(Vec::new()));
        }
        let start = cursor(&page, &streams, "search|")?;
        pager::collect(start, page.limit, Fill::Round, |stream, n| {
            search_page(&query, stream, n)
        })
    }

    fn browse(node: Option<Item>, page: PageRequest) -> Result<ItemPage> {
        let Some(node) = node else {
            return Ok(ItemPage::new(vec![
                folder("daily", "Daily 30"),
                folder("radio", "Guess You Like"),
                folder("ai", "AI Playlists"),
                folder("charts", "Charts"),
            ]));
        };
        match (node.kind, node.id.as_str()) {
            (ItemKind::Folder, "daily") => {
                let answer: SongList = call("/discover/daily-mix", json!({}))?;
                Ok(ItemPage::new(songs(answer.songlist)))
            }
            (ItemKind::Folder, "radio") => {
                let answer: SongList = call("/discover/radio", json!({}))?;
                Ok(ItemPage::new(songs(answer.songlist)))
            }
            (ItemKind::Folder, "ai") => {
                let answer: Playlists = call("/discover/ai-playlists", json!({ "reqType": "all" }))?;
                Ok(ItemPage::new(
                    list(answer.playlists)
                        .into_iter()
                        .filter_map(Playlist::item)
                        .collect(),
                ))
            }
            (ItemKind::Folder, "charts") => Ok(ItemPage::new(
                charts()?
                    .into_iter()
                    .filter(|g| !g.group_name.trim().is_empty())
                    .map(|g| folder(&format!("charts:{}", g.group_name.trim()), g.group_name.trim()))
                    .collect(),
            )),
            (ItemKind::Folder, id) if id.starts_with("charts:") => {
                let name = &id["charts:".len()..];
                let group = charts()?
                    .into_iter()
                    .find(|g| g.group_name.trim() == name)
                    .ok_or(PluginError::NotFound)?;
                Ok(ItemPage::new(
                    list(group.group_top_list)
                        .into_iter()
                        .filter_map(items::Chart::item)
                        .collect(),
                ))
            }
            (ItemKind::Folder, id) if id.starts_with("chart:") => {
                let top_id: i64 = id["chart:".len()..].parse().map_err(|_| PluginError::NotFound)?;
                let params = json!({ "topId": top_id });
                let start = cursor(&page, &[0], &tracks_key("/charts/detail", &params))?;
                pager::collect(start, page.limit, Fill::Limit, |_, n| {
                    tracks_page("/charts/detail", params.clone(), n)
                })
            }
            (ItemKind::Playlist, id) => {
                let diss_id: i64 = id.parse().map_err(|_| PluginError::NotFound)?;
                let params = json!({ "dissId": diss_id });
                let start = cursor(&page, &[0], &tracks_key("/playlists/detail", &params))?;
                pager::collect(start, page.limit, Fill::Limit, |_, n| {
                    tracks_page("/playlists/detail", params.clone(), n)
                })
            }
            (ItemKind::Album, _) => Err(PluginError::service(
                "QQ Music's open API cannot list an album's tracks. Open the album in QQ Music.",
            )),
            (ItemKind::Artist, _) => Err(PluginError::service(
                "QQ Music's open API cannot list a singer's music. Open the singer in QQ Music.",
            )),
            _ => Err(PluginError::NotFound),
        }
    }

    fn recommendations_for_user(page: PageRequest) -> Result<ItemPage> {
        // Daily 30, then Guess You Like, read afresh for every first page.
        let start = match page.cursor.as_deref() {
            Some(text) => Cursor::decode(text, 2)?,
            None => {
                pager::invalidate("for-you|");
                Cursor::start(&[0, 1])
            }
        };
        pager::collect(start, page.limit, Fill::Limit, |stream, _| {
            let path = if stream == 0 {
                "/discover/daily-mix"
            } else {
                "/discover/radio"
            };
            cached(format!("for-you|{path}"), || {
                let answer: SongList = call(path, json!({}))?;
                Ok((songs(answer.songlist), false))
            })
        })
    }
}

jf_plugin_sdk::export_plugin!(QqMusic);
