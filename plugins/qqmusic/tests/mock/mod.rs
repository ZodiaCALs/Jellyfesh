//! A mock of QQ Music's open API, written from the `qqmusic-skills` 0.0.3
//! documentation (SKILL.md, discover.md, charts.md, playlists.md,
//! version.md). Every name and id in it is invented test data, not QQ
//! Music's catalogue.
//!
//! Like the documented server it answers only `POST` with
//! `Authorization: Bearer <key>` and reads parameters only from `params`;
//! unlike it, it also refuses requests without `comm.skill_version` or with
//! business parameters at the top level, so the tests notice such mistakes.
//! What it answers for a wrong key (HTTP 401) is an assumption: the
//! documentation does not say.

use jf_plugin_testkit::{Request, Response, TestServer};
use serde_json::{Value, json};

/// The key the mock accepts.
pub const KEY: &str = "qmk-test-0123abcd";
/// A key the mock answers with HTTP 429.
pub const SLOW_KEY: &str = "qmk-test-slow";
/// A key the mock answers with a gateway error in `ret`.
pub const BROKEN_KEY: &str = "qmk-test-broken";
/// Songs per page of a chart or a playlist (the server's fixed page size).
pub const PAGE: usize = 20;

pub fn start() -> TestServer {
    TestServer::start(answer)
}

fn error(ret: i64, msg: &str) -> Response {
    Response::json(&json!({ "traceId": "mock", "ts": 1, "ret": ret, "sub_ret": 0, "msg": msg }))
}

fn ok(mut body: Value) -> Response {
    body["traceId"] = json!("mock");
    body["ts"] = json!(1_780_000_000_000_i64);
    body["ret"] = json!(0);
    body["sub_ret"] = json!(0);
    body["msg"] = json!("");
    Response::json(&body)
}

fn song(mid: &str, name: &str, singer: &str) -> Value {
    json!({
        "songMid": mid,
        "songName": name,
        "singerName": singer,
        "songH5Url": format!("https://y.qq.com/n/ryqq/songDetail/{mid}")
    })
}

/// `count` invented songs whose mids start with `prefix`.
fn songs(prefix: &str, from: usize, count: usize) -> Vec<Value> {
    (from..from + count)
        .map(|i| {
            song(
                &format!("{prefix}{i:04}"),
                &format!("Song {prefix} {i}"),
                "Lin Example",
            )
        })
        .collect()
}

/// A page of a list of `total` songs.
fn paged(prefix: &str, total: usize, page: usize) -> (Vec<Value>, bool) {
    let from = page * PAGE;
    let count = total.saturating_sub(from).min(PAGE);
    (songs(prefix, from, count), from + count < total)
}

fn answer(request: &Request) -> Response {
    if request.method != "POST" {
        return Response::text(405, "POST only");
    }
    let key = request
        .header("authorization")
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or_default();
    match key {
        KEY => {}
        SLOW_KEY => return Response::text(429, "slow down"),
        BROKEN_KEY => return error(10003, "gateway busy (mock)"),
        _ => {
            return Response::new(401, r#"{"ret":401,"msg":"unauthorized"}"#)
                .header("content-type", "application/json");
        }
    }
    let Some(body) = request.json() else {
        return error(400, "the body is not JSON");
    };
    if body["comm"]["skill_version"] != "0.0.3" {
        return error(400, "comm.skill_version is missing");
    }
    let Some(params) = body["params"].as_object() else {
        return error(400, "params is missing");
    };
    if body
        .as_object()
        .is_some_and(|o| o.keys().any(|k| k != "params" && k != "comm"))
    {
        return error(400, "business parameters belong in params");
    }
    let param = |name: &str| params.get(name).cloned().unwrap_or(Value::Null);
    let page = param("page").as_u64().unwrap_or(0) as usize;
    match request.path() {
        "/skill/version" => ok(json!({ "upgradeInfo": { "needUpgrade": false, "latestVersion": "0.0.3" } })),
        "/discover/search" => {
            let keyword = param("keyword").as_str().unwrap_or_default().to_owned();
            let kind = param("type").as_str().unwrap_or("0").to_owned();
            let mut body = json!({ "songs": null, "albums": null, "playlists": null, "mvs": null, "radios": null, "singers": null });
            let more = match kind.as_str() {
                "0" => {
                    let (list, more) = paged("0039Search", 2 * PAGE, page);
                    body["songs"] = json!(list);
                    more
                }
                "1" => {
                    body["albums"] = json!([
                        { "albumMid": "002AlbumA", "albumName": format!("{keyword} (Album)"), "singerName": "Lin Example",
                          "publicTime": "2019-04-02", "listenNum": 1200, "category": "Pop", "albumDesc": "Invented." },
                        { "albumMid": "", "albumName": "no mid" }
                    ]);
                    false
                }
                "2" => {
                    body["playlists"] = json!([
                        { "dissId": 7_100_000_001_i64, "dissName": format!("{keyword} for late nights"), "dissDesc": "", "creatorName": "Mock Curator" }
                    ]);
                    false
                }
                "5" => {
                    body["singers"] = json!([{ "singerMid": "001SingerA", "singerName": "Lin Example", "songNum": 40, "albumNum": 3 }]);
                    false
                }
                _ => false,
            };
            body["hasMore"] = json!(more);
            ok(body)
        }
        "/discover/daily-mix" => ok(json!({ "songlist": songs("0039Daily", 0, 30) })),
        "/discover/radio" => ok(json!({ "songlist": songs("0039Radio", 0, 10) })),
        "/discover/ai-playlists" if param("reqType") == "all" => ok(json!({
            "playlists": [
                { "dissId": 7_100_000_002_i64, "dissName": "Rainy Commute (AI)", "dissDesc": "Invented.", "creatorName": "" },
                { "dissId": 7_100_000_003_i64, "dissName": "Focus Loops (AI)", "dissDesc": "Invented.", "creatorName": "" }
            ]
        })),
        "/charts" => ok(json!({
            "groupList": [
                { "groupName": "Peak Charts", "groupTopList": [
                    { "topId": 26, "topName": "Hot Songs", "listenNum": 1000, "totalNum": 45,
                      "songList": [{ "rank": 1, "songName": "Song 0039Chart 0", "singerName": "Lin Example" }] },
                    { "topId": 27, "topName": "New Songs", "listenNum": 900, "totalNum": 5, "songList": [] }
                ] },
                { "groupName": "Regional Charts", "groupTopList": [
                    { "topId": 3, "topName": "Harbour City", "listenNum": 10, "totalNum": 5, "songList": [] }
                ] }
            ]
        })),
        "/charts/detail" => {
            let top = param("topId").as_i64().unwrap_or(-1);
            let total = match top {
                26 => 45,
                27 | 3 => 5,
                _ => return error(2001, "no such chart (mock)"),
            };
            let (list, more) = paged(&format!("0039Chart{top}x"), total, page);
            ok(json!({
                "topId": top, "topType": 0, "topName": "Hot Songs", "topDesc": "Invented.",
                "topHeaderPic": "https://example.invalid/header.jpg", "topBannerPic": "https://example.invalid/banner.jpg",
                "listenNum": 1000, "totalNum": total, "updateTime": "2026-11-16", "hasMore": more, "trackList": list
            }))
        }
        "/playlists/detail" => {
            let diss = param("dissId").as_i64().unwrap_or(-1);
            let total = match diss {
                7_100_000_001 => 3,
                7_100_000_002 => 25,
                7_100_000_003 => 0,
                _ => return error(2002, "no such playlist (mock)"),
            };
            let (list, more) = paged(&format!("0039List{diss}x"), total, page);
            ok(json!({
                "dissId": diss, "dissName": "Invented playlist", "desc": "", "picUrl": "https://example.invalid/cover.jpg",
                "totalNum": total, "hasMore": more, "trackList": list
            }))
        }
        _ => error(404, "unknown path (mock)"),
    }
}
