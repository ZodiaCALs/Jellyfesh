//! Discover and details (plugin API 1.2, ADR-0022), from the web player's
//! own calls on u.y.qq.com:
//!
//! * Discover: recommended playlists (`playlist.HotRecommendServer`
//!   `get_hot_recommend`, the home page's playlists; the account's own
//!   picks when signed in) and the charts (`musicToplist.ToplistInfoServer`
//!   `GetAll`). A chart (`qqm:top:<id>`) lists its songs through `GetDetail`
//!   for its current period, which `GetAll` names.
//! * Details: a song's language, genre, release date, lyricist, composer
//!   and notes (the `info` of `music.pf_song_detail_svr`
//!   `get_song_detail_yqq`), an album's notes, release date, genre,
//!   language and company (`music.musichallAlbum.AlbumInfoServer`
//!   `GetAlbumDetail`), a playlist's description, tags and listens
//!   (`music.srfDissInfo.aiDissInfo` `uniform_get_Dissinfo`), and a chart's
//!   introduction, listens and last update.
//! * No comments: QQ Music's comment calls could not be checked from where
//!   the plugin was written. No word timing: QQ Music sends it only as the
//!   app's encrypted QRC, which the plugin does not decrypt.

use std::cell::RefCell;

use jf_community_common::{entities, paging};
use jf_plugin_sdk::prelude::*;
use serde_json::{Value, json};

use crate::api;
use crate::items::{self, Id};
use crate::model::{self, list, number, text};

/// Items on a Discover shelf.
const SHELF: usize = 30;
/// The large picture of details.
const LARGE: u32 = 800;
/// How long the chart list is kept (ms).
const CHARTS_TTL_MS: u64 = 10 * 60 * 1000;

thread_local! {
    /// The chart list and when it was read.
    static CHARTS: RefCell<Option<(u64, Vec<Chart>)>> = const { RefCell::new(None) };
}

/// One chart of `GetAll`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chart {
    pub id: String,
    pub title: String,
    pub period: String,
    pub picture: Option<String>,
    pub intro: Option<String>,
    pub listens: Option<u64>,
    pub updated: Option<String>,
}

fn clean(text: Option<String>) -> Option<String> {
    text.map(|t| entities::decode(&t).trim().to_owned())
        .filter(|t| !t.is_empty())
}

/// The charts in `GetAll`'s answer, group by group.
pub fn read_charts(data: &Value) -> Vec<Chart> {
    list(data, "group")
        .iter()
        .flat_map(|group| list(group, "toplist"))
        .filter_map(|entry| {
            let id = text(entry, &["topId", "topid", "id"])?;
            if !id.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            Some(Chart {
                title: clean(text(entry, &["title", "topTitle", "name"]))
                    .unwrap_or_else(|| format!("Chart {id}")),
                period: text(entry, &["period"]).unwrap_or_default(),
                picture: text(entry, &["frontPicUrl", "headPicUrl", "mbFrontPicUrl", "picUrl"]),
                intro: clean(text(entry, &["intro", "titleDetail"])),
                listens: number(entry, &["listenNum", "listennum"]),
                updated: text(entry, &["updateTime", "update_time"]),
                id,
            })
        })
        .collect()
}

/// The charts, read at most every ten minutes.
fn charts() -> Result<Vec<Chart>> {
    let now = clock::now_utc_ms();
    if let Some(kept) = CHARTS.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(at, _)| now.saturating_sub(*at) < CHARTS_TTL_MS)
            .map(|(_, charts)| charts.clone())
    }) {
        return Ok(kept);
    }
    let data = api::call("musicToplist.ToplistInfoServer", "GetAll", json!({}))?;
    let charts = read_charts(&data);
    CHARTS.with(|c| *c.borrow_mut() = Some((now, charts.clone())));
    Ok(charts)
}

/// `qqm:top:<id>`.
pub fn chart_id(id: &str) -> String {
    format!("qqm:top:{id}")
}

fn chart_item(chart: &Chart) -> Item {
    let mut item = Item::new(ItemKind::Playlist, chart_id(&chart.id), chart.title.clone());
    item.artist = Some("QQ Music".into());
    item.artwork = chart.picture.as_deref().and_then(api::picture);
    item
}

/// The recommended playlists.
fn recommended() -> Result<Vec<Item>> {
    let data = api::call(
        "playlist.HotRecommendServer",
        "get_hot_recommend",
        json!({ "async": 1, "cmd": 2 }),
    )?;
    Ok(list(&data, "v_hot")
        .iter()
        .filter_map(|entry| {
            let id = text(entry, &["content_id", "dissid", "tid"])?;
            if !id.bytes().all(|b| b.is_ascii_digit()) || id == "0" {
                return None;
            }
            let title =
                clean(text(entry, &["title", "dissname"])).unwrap_or_else(|| format!("Playlist {id}"));
            let mut item = Item::new(ItemKind::Playlist, items::playlist_id(&id), title);
            item.artist = clean(text(entry, &["username", "creator"]));
            let cover = text(entry, &["cover", "imgurl", "picurl"]);
            api::remember_cover(&item.id, cover.as_deref());
            item.artwork = cover.as_deref().and_then(api::picture);
            Some(item)
        })
        .collect())
}

/// A page of a chart's songs, for its current period.
pub fn chart_page(id: &str, page: &PageRequest) -> Result<ItemPage> {
    let chart = charts()?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or(PluginError::NotFound)?;
    let offset = paging::offset(page)?;
    let limit = paging::limit(page);
    let data = api::call(
        "musicToplist.ToplistInfoServer",
        "GetDetail",
        json!({ "topId": id.parse::<u64>().unwrap_or(0), "offset": offset, "num": limit, "period": chart.period }),
    )?;
    let songs = model::tracks(data.get("songInfoList").unwrap_or(&Value::Null));
    let total = data
        .get("data")
        .and_then(|d| number(d, &["totalNum", "total"]))
        .map(|t| t as usize);
    Ok(paging::served(items::tracks(&songs), offset, limit, total))
}

/// A See All folder of Discover, listed in full; `None` for other folders.
pub fn browse_folder(name: &str, page: &PageRequest) -> Option<Result<ItemPage>> {
    let items = match name {
        "recommended" => recommended(),
        "charts" => charts().map(|charts| charts.iter().map(chart_item).collect()),
        _ => return None,
    };
    Some(items.and_then(|items| paging::slice(items, page)))
}

/// The Discover page. A shelf whose list fails is left out; the sign-in,
/// the rate limit and the network fail the page.
pub fn discover() -> Result<Vec<Shelf>> {
    type Fetch = fn() -> Result<Vec<Item>>;
    let lists: [(&str, &str, ShelfStyle, Fetch); 2] = [
        (
            "recommended",
            "Recommended Playlists",
            ShelfStyle::Grid,
            recommended,
        ),
        ("charts", "Charts", ShelfStyle::Grid, || {
            charts().map(|charts| charts.iter().map(chart_item).collect())
        }),
    ];
    let mut shelves = Vec::new();
    for (name, title, style, fetch) in lists {
        match fetch() {
            Ok(mut found) if !found.is_empty() => {
                found.truncate(SHELF);
                shelves.push(Shelf {
                    id: name.into(),
                    title: title.into(),
                    style,
                    items: found,
                    more: Some(items::folder(name, title).id),
                });
            }
            Ok(_) => {}
            Err(error @ (PluginError::AuthRequired | PluginError::RateLimited | PluginError::Network(_))) => {
                return Err(error);
            }
            Err(error) => jf_plugin_sdk::warn!("no {title} for Discover: {}", crate::error_text(&error)),
        }
    }
    Ok(shelves)
}

fn fact(key: FactKey, value: Option<String>) -> Option<Fact> {
    let value = clean(value)?;
    Some(Fact { key, value })
}

/// The text of one `info` entry of a song's details: its `content` values.
fn info_value(entry: &Value) -> Option<String> {
    let values: Vec<String> = list(entry, "content")
        .iter()
        .filter_map(|c| text(c, &["value", "name"]))
        .map(|v| entities::decode(&v))
        .collect();
    (!values.is_empty()).then(|| values.join(", "))
}

/// Facts and notes from the `info` list of `get_song_detail_yqq`, by the
/// entries' `type`: `lan`, `genre`, `pub_time`, `lyric` (lyricist),
/// `composer`, `company`, `intro`.
pub fn song_info(info: &[Value]) -> (Vec<Fact>, Option<String>) {
    let mut facts = Vec::new();
    let mut notes = None;
    for entry in info {
        let Some(value) = info_value(entry) else {
            continue;
        };
        let key = match text(entry, &["type"]).as_deref() {
            Some("lan") => FactKey::Language,
            Some("genre") => FactKey::Genre,
            Some("pub_time") => FactKey::Released,
            Some("lyric") => FactKey::Lyricist,
            Some("composer") => FactKey::Composer,
            Some("company") => FactKey::Label,
            Some("intro") => {
                notes = Some(value);
                continue;
            }
            _ => continue,
        };
        facts.extend(fact(key, Some(value)));
    }
    (facts, notes)
}

fn song_details(mid: &str) -> Result<ItemDetails> {
    let data = api::call(
        "music.pf_song_detail_svr",
        "get_song_detail_yqq",
        json!({ "song_type": 0, "song_mid": mid }),
    )?;
    let track = data
        .get("track_info")
        .and_then(model::track)
        .ok_or(PluginError::NotFound)?;
    let (facts, notes) = song_info(list(&data, "info"));
    Ok(ItemDetails {
        description: notes,
        image: api::album_picture(&track.album.mid, LARGE),
        facts,
        comment_count: None,
    })
}

fn album_details(mid: &str) -> Result<ItemDetails> {
    let data = api::call(
        "music.musichallAlbum.AlbumInfoServer",
        "GetAlbumDetail",
        json!({ "albumMid": mid }),
    )?;
    let basic = data.get("basicInfo").ok_or(PluginError::NotFound)?;
    let company = data.get("company").and_then(|c| text(c, &["name"]));
    Ok(ItemDetails {
        description: clean(text(basic, &["desc", "albumDesc"])),
        image: api::album_picture(mid, LARGE),
        facts: [
            fact(FactKey::Released, text(basic, &["publishDate", "pubTime"])),
            fact(FactKey::Genre, text(basic, &["genre"])),
            fact(FactKey::Language, text(basic, &["language"])),
            fact(FactKey::Other("Type".into()), text(basic, &["albumType"])),
            fact(FactKey::Label, company),
        ]
        .into_iter()
        .flatten()
        .collect(),
        comment_count: None,
    })
}

/// `1234567` as `1,234,567`.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn listens(n: Option<u64>) -> Option<Fact> {
    fact(
        FactKey::Popularity,
        n.filter(|n| *n > 0).map(|n| format!("{} listens", thousands(n))),
    )
}

fn playlist_details(id: &str) -> Result<ItemDetails> {
    let data = api::call(
        "music.srfDissInfo.aiDissInfo",
        "uniform_get_Dissinfo",
        json!({
            "disstid": id.parse::<u64>().unwrap_or(0), "userinfo": 1, "tag": 1, "orderlist": 0,
            "song_begin": 0, "song_num": 1, "onlysonglist": 0, "enc_host_uin": "",
        }),
    )?;
    let info = data.get("dirinfo").ok_or(PluginError::NotFound)?;
    let tags: Vec<String> = list(info, "tag")
        .iter()
        .filter_map(|t| text(t, &["name"]))
        .map(|t| entities::decode(&t))
        .collect();
    Ok(ItemDetails {
        description: clean(text(info, &["desc", "introduction"])),
        image: text(info, &["picurl", "picUrl", "cover"])
            .as_deref()
            .and_then(api::picture),
        facts: [
            listens(number(info, &["listennum", "listenNum"])),
            fact(FactKey::Other("Tags".into()), Some(tags.join(", "))),
        ]
        .into_iter()
        .flatten()
        .collect(),
        comment_count: None,
    })
}

fn chart_details(id: &str) -> Result<ItemDetails> {
    let chart = charts()?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or(PluginError::NotFound)?;
    Ok(ItemDetails {
        description: chart.intro.clone(),
        image: chart.picture.as_deref().and_then(api::picture),
        facts: [
            listens(chart.listens),
            fact(FactKey::Other("Updated".into()), chart.updated.clone()),
        ]
        .into_iter()
        .flatten()
        .collect(),
        comment_count: None,
    })
}

/// Details of one of the plugin's items.
pub fn details(item: &Item) -> Result<ItemDetails> {
    if let Some(chart) = item.id.strip_prefix("qqm:top:") {
        return chart_details(chart);
    }
    match Id::parse(&item.id) {
        Some(Id::Track(mid)) => song_details(mid),
        Some(Id::Album(mid)) => album_details(mid),
        Some(Id::Playlist(id)) => playlist_details(id),
        // Singers' pages could not be checked; folders have nothing to say.
        Some(Id::Artist(_) | Id::Folder(_)) => Err(PluginError::Unsupported),
        None => Err(PluginError::NotFound),
    }
}

/// Whether `id` is one of the plugin's charts; browse lists its songs.
pub fn chart_of(id: &str) -> Option<&str> {
    id.strip_prefix("qqm:top:")
        .filter(|n| !n.is_empty() && n.len() <= 12 && n.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charts_come_group_by_group() {
        let data = json!({ "group": [
            { "groupId": 0, "groupName": "Mock group", "toplist": [
                { "topId": 62, "title": "Mock Soaring", "period": "2026-11-19", "frontPicUrl": "http://y.gtimg.cn/music/c.jpg",
                  "intro": "Invented &amp; climbing", "listenNum": 1234, "updateTime": "2026-11-19" },
                { "topId": "x", "title": "Not a chart" }
            ] },
            { "toplist": [ { "topId": 26, "title": "Mock Hot", "period": "2026_46" } ] }
        ] });
        let charts = read_charts(&data);
        let ids: Vec<&str> = charts.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["62", "26"]);
        assert_eq!(charts[0].intro.as_deref(), Some("Invented & climbing"));
        assert_eq!(
            (charts[0].listens, charts[1].period.as_str()),
            (Some(1234), "2026_46")
        );
        assert_eq!(chart_of("qqm:top:62"), Some("62"));
        assert_eq!(chart_of("qqm:top:../x"), None);
    }

    #[test]
    fn song_info_by_type() {
        let info = vec![
            json!({ "type": "lan", "title": "语种", "content": [{ "value": "国语" }] }),
            json!({ "type": "genre", "content": [{ "value": "Pop" }, { "value": "Folk" }] }),
            json!({ "type": "pub_time", "content": [{ "value": "2021-03-05" }] }),
            json!({ "type": "lyric", "content": [{ "value": "Mock Writer" }] }),
            json!({ "type": "composer", "content": [{ "value": "Mock Composer" }] }),
            json!({ "type": "intro", "content": [{ "value": "Invented notes &amp; more." }] }),
            json!({ "type": "unknown", "content": [{ "value": "ignored" }] }),
            json!({ "type": "company", "content": [] }),
        ];
        let (facts, notes) = song_info(&info);
        let facts: Vec<(FactKey, String)> = facts.into_iter().map(|f| (f.key, f.value)).collect();
        assert_eq!(
            facts,
            [
                (FactKey::Language, "国语".to_string()),
                (FactKey::Genre, "Pop, Folk".into()),
                (FactKey::Released, "2021-03-05".into()),
                (FactKey::Lyricist, "Mock Writer".into()),
                (FactKey::Composer, "Mock Composer".into()),
            ]
        );
        assert_eq!(notes.as_deref(), Some("Invented notes & more."));
    }
}
