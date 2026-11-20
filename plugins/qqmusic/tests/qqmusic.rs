//! The QQ Music plugin's component in a real plugin host, against a mock of
//! QQ Music's open API (`tests/mock`, invented data). The service itself was
//! not reachable from where these tests were written: they check the
//! protocol as documented, not the live service.

mod mock;

use std::collections::HashSet;
use std::path::Path;

use jf_plugin_host::{AuthKind, AuthStep, CallError, ItemKind, PluginError, SecretStore, UnplayableReason};
use jf_plugin_testkit::*;
use serde_json::json;

const ID: &str = "org.jellyfesh.qqmusic";

fn crate_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The plugin installed with a test package pointing at `server`.
fn install(server: &TestServer) -> Env {
    let env = Env::new();
    let files = PluginFiles::read(crate_dir()).with_test_server_setting("test_server");
    let info = env.install_files(crate_dir(), "jf-qqmusic", &files);
    assert_eq!(info.id, ID);
    env.settings(ID, json!({ "test_server": server.url() }));
    env
}

fn sign_in(env: &Env, key: &str) -> AuthStep {
    let flow = login(&env.host, ID, AuthKind::ApiKey);
    let AuthStep::AskFields { fields: form } = next_step(&flow) else {
        panic!("expected the key form");
    };
    assert_eq!(form.len(), 1);
    flow.submit(fields(&[(form[0].name.as_str(), key)]));
    final_step(&flow)
}

fn signed_in(server: &TestServer) -> Env {
    let env = install(server);
    assert!(matches!(sign_in(&env, mock::KEY), AuthStep::Done { .. }));
    server.clear();
    env
}

fn ids(page: &jf_plugin_host::ItemPage) -> Vec<String> {
    page.items.iter().map(|i| i.id.clone()).collect()
}

#[test]
fn signs_in_with_the_persons_own_key_checked_by_a_cheap_call() {
    let server = mock::start();
    let env = install(&server);
    let flow = login(&env.host, ID, AuthKind::ApiKey);
    let AuthStep::AskFields { fields: form } = next_step(&flow) else {
        panic!("expected the key form");
    };
    assert_eq!(form[0].name, "api-key");
    assert!(form[0].secret);
    let help = form[0].help.clone().unwrap_or_default();
    assert!(help.contains("y.qq.com/n/ryqq_v2/qqmusic_skills"), "{help}");
    assert!(help.contains("personal use"), "{help}");
    flow.submit(fields(&[("api-key", "not a key")]));
    assert!(matches!(final_step(&flow), AuthStep::Failed { message } if message.contains("qmk-")));
    assert!(
        server.requests().is_empty(),
        "a malformed key is not sent anywhere"
    );

    let refused = sign_in(&env, "qmk-test-wrong");
    assert!(
        matches!(&refused, AuthStep::Failed { message } if message.contains("did not accept")),
        "{refused:?}"
    );
    assert_eq!(
        sign_in(&env, &format!("  {}\n", mock::KEY)),
        AuthStep::Done {
            account: "API key …abcd".into()
        }
    );
    let check = server.requests_to("/skill/version").pop().unwrap();
    assert_eq!(
        check.header("authorization"),
        Some(format!("Bearer {}", mock::KEY).as_str())
    );
    assert!(
        check
            .header("content-type")
            .unwrap_or_default()
            .starts_with("application/json")
    );
    assert_eq!(
        check.json().unwrap(),
        json!({ "params": {}, "comm": { "skill_version": "0.0.3" } })
    );
    assert!(
        env.secrets.any_value_contains(mock::KEY),
        "the key is in the keystore"
    );
    assert!(env.files_containing(mock::KEY).is_empty(), "and in no file");
    assert_eq!(
        env.host.plugin(ID).unwrap().account.as_deref(),
        Some("API key …abcd")
    );
}

#[test]
fn searches_every_kind_with_the_parameters_inside_params() {
    let server = mock::start();
    let env = signed_in(&server);
    let found = recv(env.host.search(ID, "night", kinds(&[]), page(50))).unwrap();
    let found_kinds: Vec<ItemKind> = found.items.iter().map(|i| i.kind).collect();
    // One server page of each type: songs (the server's page size), then the
    // singer, album and playlist; more songs on the next page.
    assert_eq!(found.items.len(), mock::PAGE + 3);
    assert!(found_kinds[..mock::PAGE].iter().all(|k| *k == ItemKind::Track));
    assert_eq!(
        &found_kinds[mock::PAGE..],
        [ItemKind::Artist, ItemKind::Album, ItemKind::Playlist]
    );
    assert!(found.next.is_some());
    let album = &found.items[mock::PAGE + 1];
    assert_eq!((album.id.as_str(), album.year), ("002AlbumA", Some(2019)));
    let playlist = &found.items[mock::PAGE + 2];
    assert_eq!(playlist.id, "7100000001");
    assert_eq!(playlist.artist.as_deref(), Some("Mock Curator"));
    for item in &found.items {
        assert!(!item.playable, "{} plays nowhere here", item.id);
        let reason = item.unplayable.as_ref().unwrap();
        assert_eq!(reason.reason, UnplayableReason::Other);
        assert_eq!(
            reason.message.as_deref(),
            Some("QQ Music's open API does not stream; open it in QQ Music.")
        );
    }
    // Every item opens in QQ Music on the documented pages.
    let links = env.host.plugin(ID).unwrap().links;
    assert_eq!(
        links.url_for(ItemKind::Track, &found.items[0].id).as_deref(),
        Some("https://i2.y.qq.com/a/song/0039Search0000")
    );
    assert_eq!(
        links.url_for(ItemKind::Artist, "001SingerA").as_deref(),
        Some("https://i2.y.qq.com/a/singer/001SingerA")
    );
    assert_eq!(
        links.url_for(ItemKind::Playlist, &playlist.id).as_deref(),
        Some("https://i2.y.qq.com/a/playlist/7100000001")
    );
    // One request per type, each with the parameters nested in params.
    let requests = server.requests_to("/discover/search");
    let types: Vec<String> = requests
        .iter()
        .map(|r| r.json().unwrap()["params"]["type"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(types, ["0", "5", "1", "2"]);
    for r in &requests {
        let body = r.json().unwrap();
        assert_eq!(body["params"]["keyword"], "night");
        assert_eq!(body["params"]["page"], 0);
        assert_eq!(body["comm"]["skill_version"], "0.0.3");
        assert!(body.get("keyword").is_none());
    }
    // Songs only, in host pages smaller than the server's: all 40, once each.
    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = recv(
            env.host
                .search(ID, "night", kinds(&[ItemKind::Track]), next_page(cursor, 15)),
        )
        .unwrap();
        assert!(page.items.len() <= 15);
        seen.extend(ids(&page));
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen.len(), 2 * mock::PAGE);
    assert_eq!(seen.iter().collect::<HashSet<_>>().len(), seen.len());
}

#[test]
fn browses_daily_30_charts_and_playlists() {
    let server = mock::start();
    let env = signed_in(&server);
    let root = recv(env.host.browse(ID, None, page(100))).unwrap();
    let titles: Vec<&str> = root.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(titles, ["Daily 30", "Guess You Like", "AI Playlists", "Charts"]);
    assert!(root.items.iter().all(|i| i.kind == ItemKind::Folder));

    let daily = recv(env.host.browse(ID, Some(root.items[0].clone()), page(100))).unwrap();
    assert_eq!(daily.items.len(), 30);
    assert!(
        daily
            .items
            .iter()
            .all(|i| i.kind == ItemKind::Track && !i.playable)
    );

    // Charts → groups → charts → the chart's songs, page after page.
    let groups = recv(env.host.browse(ID, Some(root.items[3].clone()), page(100))).unwrap();
    let names: Vec<&str> = groups.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(names, ["Peak Charts", "Regional Charts"]);
    let charts = recv(env.host.browse(ID, Some(groups.items[0].clone()), page(100))).unwrap();
    assert_eq!(ids(&charts), ["chart:26", "chart:27"]);
    assert_eq!(charts.items[0].child_count, Some(45));
    let first = recv(env.host.browse(ID, Some(charts.items[0].clone()), page(100))).unwrap();
    assert_eq!(first.items.len(), 45, "three server pages fill one host page");
    assert_eq!(first.next, None);
    let pages: Vec<u64> = server
        .requests_to("/charts/detail")
        .iter()
        .map(|r| {
            let body = r.json().unwrap();
            assert_eq!(body["params"]["topId"], 26);
            body["params"]["page"].as_u64().unwrap()
        })
        .collect();
    assert_eq!(pages, [0, 1, 2]);

    // AI playlists → a playlist's songs; the playlist id is the bare dissId.
    let ai = recv(env.host.browse(ID, Some(root.items[2].clone()), page(100))).unwrap();
    assert_eq!(ids(&ai), ["7100000002", "7100000003"]);
    assert!(ai.items.iter().all(|i| i.kind == ItemKind::Playlist));
    let songs = recv(env.host.browse(ID, Some(ai.items[0].clone()), page(10))).unwrap();
    assert_eq!(songs.items.len(), 10);
    let rest = recv(
        env.host
            .browse(ID, Some(ai.items[0].clone()), next_page(songs.next, 100)),
    )
    .unwrap();
    assert_eq!(rest.items.len(), 15);
    assert_eq!(rest.next, None);
    let detail = server.requests_to("/playlists/detail");
    assert_eq!(detail[0].json().unwrap()["params"]["dissId"], 7_100_000_002_i64);
    let empty = recv(env.host.browse(ID, Some(ai.items[1].clone()), page(10))).unwrap();
    assert!(empty.items.is_empty());
}

#[test]
fn albums_and_singers_say_to_open_them_in_qq_music() {
    let server = mock::start();
    let env = signed_in(&server);
    for (kind, word) in [(ItemKind::Album, "album"), (ItemKind::Artist, "singer")] {
        match recv(
            env.host
                .browse(ID, Some(item(kind, "002AlbumA", "Something")), page(50)),
        ) {
            Err(CallError::Plugin(PluginError::Service(message))) => {
                assert!(
                    message.contains(&format!("Open the {word} in QQ Music")),
                    "{message}"
                );
            }
            other => panic!("expected a service error, got {other:?}"),
        }
    }
    assert!(
        server.requests().is_empty(),
        "no request for what the API cannot list"
    );
}

#[test]
fn recommendations_are_daily_30_then_guess_you_like() {
    let server = mock::start();
    let env = signed_in(&server);
    let first = recv(env.host.recommendations_for_user(ID, page(25))).unwrap();
    assert_eq!(first.items.len(), 25);
    assert!(first.items[0].id.starts_with("0039Daily"));
    let second = recv(env.host.recommendations_for_user(ID, next_page(first.next, 100))).unwrap();
    assert_eq!(
        second.items.len(),
        5 + 10,
        "the rest of Daily 30, then Guess You Like"
    );
    assert!(second.items[5].id.starts_with("0039Radio"));
    assert_eq!(second.next, None);
    assert_eq!(
        server.count("/discover/daily-mix"),
        1,
        "the split page came from the cache"
    );
    // A new first page asks again.
    recv(env.host.recommendations_for_user(ID, page(5))).unwrap();
    assert_eq!(server.count("/discover/daily-mix"), 2);
}

#[test]
fn service_answers_map_to_the_hosts_errors() {
    let server = mock::start();
    let env = install(&server);
    // Not signed in.
    assert!(matches!(
        recv(env.host.search(ID, "x", kinds(&[]), page(10))),
        Err(CallError::Plugin(PluginError::AuthRequired))
    ));
    // A key the service stops accepting, one it throttles, and a gateway error.
    for (key, expected) in [
        ("qmk-test-revoked", "auth"),
        (mock::SLOW_KEY, "slow"),
        (mock::BROKEN_KEY, "busy"),
    ] {
        env.secrets.set(&format!("jellyfesh/{ID}/api-key"), key).unwrap();
        let result = recv(env.host.search(ID, "x", kinds(&[ItemKind::Track]), page(10)));
        match (expected, result) {
            ("auth", Err(CallError::Plugin(PluginError::AuthRequired))) => {}
            ("slow", Err(CallError::Plugin(PluginError::RateLimited))) => {}
            ("busy", Err(CallError::Plugin(PluginError::Service(message)))) => {
                assert_eq!(message, "gateway busy (mock)");
            }
            (_, other) => panic!("{key}: unexpected {other:?}"),
        }
    }
}

#[test]
fn the_published_package_reaches_only_qq_music() {
    let files = PluginFiles::read(crate_dir());
    let manifest = jf_plugin_host::manifest::parse(&files.manifest).unwrap();
    assert_eq!(manifest.api_version.to_string(), "1.2");
    assert_eq!(manifest.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest.max_rps, 2);
    let host = |host: &str| jf_plugin_host::manifest::AllowRule::Host {
        host: host.into(),
        wildcard: false,
        port: 443,
    };
    assert_eq!(
        manifest.allow,
        [host("a.y.qq.com"), host("y.gtimg.cn"), host("qpic.y.qq.com")],
        "the API, and its pictures"
    );
    assert!(!manifest.content_policy.unified_search);
    assert_eq!(
        manifest.content_policy.library_storage,
        jf_plugin_host::LibraryStorage::None
    );
    assert_eq!(
        manifest.links.url_for(ItemKind::Album, "002AlbumA").as_deref(),
        Some("https://i2.y.qq.com/a/album/002AlbumA")
    );
    // The real package has no test server setting, so the host refuses one.
    let env = Env::new();
    env.install_crate(crate_dir(), "jf-qqmusic");
    assert!(
        env.host
            .set_settings(ID, &json!({ "test_server": "http://127.0.0.1:9" }).to_string())
            .is_err()
    );
}

#[test]
fn discover_shows_the_lists_the_api_documents() {
    let server = mock::start();
    let env = signed_in(&server);
    let shelves = recv(env.host.discover(ID)).unwrap();
    let titles: Vec<&str> = shelves.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Daily 30",
            "Guess You Like",
            "AI Playlists",
            "Hot Songs",
            "Charts"
        ]
    );
    assert_eq!(shelves[0].items.len(), 30);
    assert_eq!(shelves[0].more.as_deref(), Some("daily"));
    assert!(shelves[0].items.iter().all(|i| !i.playable), "nothing plays here");
    assert_eq!(shelves[3].style, jf_plugin_host::ShelfStyle::Chart);
    assert_eq!(shelves[3].more.as_deref(), Some("chart:26"));
    assert_eq!(shelves[3].items.len(), mock::PAGE);
    let charts: Vec<&str> = shelves[4].items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(charts, ["chart:26", "chart:27", "chart:3"]);
    let paths: Vec<String> = server.requests().iter().map(|r| r.path().to_string()).collect();
    assert_eq!(
        paths,
        [
            "/discover/daily-mix",
            "/discover/radio",
            "/discover/ai-playlists",
            "/charts",
            "/charts/detail"
        ]
    );
    // See All opens the same lists.
    let daily = recv(
        env.host
            .browse(ID, Some(item(ItemKind::Folder, "daily", "Daily 30")), page(50)),
    )
    .unwrap();
    assert_eq!(daily.items.len(), 30);
}

#[test]
fn details_of_playlists_and_charts() {
    use jf_plugin_host::FactKey;
    let server = mock::start();
    let env = signed_in(&server);
    let playlist = recv(
        env.host
            .details(ID, item(ItemKind::Playlist, "7100000002", "Rainy Commute (AI)")),
    )
    .unwrap();
    assert_eq!(
        playlist.description.as_deref(),
        Some("Invented songs for a rainy commute.")
    );
    assert_eq!(
        playlist.image.map(|i| i.url).as_deref(),
        Some("https://qpic.y.qq.com/mock/playlist-cover/300"),
        "the picture host's http link, as https"
    );
    let chart = recv(
        env.host
            .details(ID, item(ItemKind::Folder, "chart:26", "Hot Songs")),
    )
    .unwrap();
    assert_eq!(chart.description.as_deref(), Some("Invented."));
    let facts: Vec<(FactKey, String)> = chart
        .facts
        .iter()
        .map(|f| (f.key.clone(), f.value.clone()))
        .collect();
    assert_eq!(
        facts,
        [
            (FactKey::Popularity, "1,000 listens".to_string()),
            (FactKey::Other("Updated".into()), "2026-11-16".into())
        ]
    );
    assert_eq!(
        chart.image.map(|i| i.url).as_deref(),
        Some("https://y.gtimg.cn/mock/chart-header.jpg")
    );
    let asked = server.requests_to("/charts/detail");
    assert_eq!(asked.last().unwrap().json().unwrap()["params"]["page"], 0);
    // The API describes no song, album or singer.
    assert_eq!(
        recv(env.host.details(ID, item(ItemKind::Track, "0039Daily0000", "x"))),
        Err(CallError::Plugin(PluginError::Unsupported))
    );
    assert_eq!(
        recv(
            env.host
                .details(ID, item(ItemKind::Playlist, "not-a-number", "x"))
        ),
        Err(CallError::Plugin(PluginError::NotFound))
    );
}
