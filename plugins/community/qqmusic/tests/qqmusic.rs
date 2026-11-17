//! The QQ Music plugin: the real component in the real Jellyfesh plugin
//! host, installed unsigned as a community plugin, against a mock of the
//! web player's API and of WeChat's web QR sign-in (`mock/mod.rs`, invented
//! data in `fixtures/`).
//!
//! The live services were never reachable from where these tests were
//! written: they show the plugin speaks the protocol as its author
//! understands it, not that QQ Music or WeChat answer that way today (see
//! the README).

mod mock;

use std::path::Path;

use jf_community_testsupport::{
    Env, component, fetch, final_step, install_community, item, kinds, login, next_page, next_step, page,
    production_files, recv, with_mock,
};
use jf_plugin_host::{
    AuthKind, AuthStep, CallError, Capability, HostError, Item, ItemKind, LibraryStorage, LyricFormat,
    PluginError, Quality, SecretStore, SignatureStatus, UnplayableReason,
};
use mock::{AUDIO, Mock, PICTURE};
use serde_json::json;

const ID: &str = "community.unofficial.qqmusic";
const PACKAGE: &str = "jf-community-qqmusic";
const SECRET: &str = "jellyfesh/community.unofficial.qqmusic/session";

fn dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn env_for(mock: &Mock) -> Env {
    with_mock(dir(), PACKAGE, &mock.server, json!({})).0
}

/// A mock whose WeChat sign-in is confirmed on the first poll.
fn quick_mock() -> Mock {
    Mock::with(|state| state.wechat_script = vec![405])
}

fn sign_in(env: &Env) {
    let flow = login(&env.host, ID, AuthKind::Qr);
    assert!(matches!(next_step(&flow), AuthStep::ShowQr { .. }));
    assert_eq!(
        final_step(&flow),
        AuthStep::Done {
            account: "Mock QQ Listener".into()
        }
    );
}

fn ids(page: &jf_plugin_host::ItemPage) -> Vec<String> {
    page.items.iter().map(|i| i.id.clone()).collect()
}

fn track(mid: &str) -> Item {
    item(ItemKind::Track, &format!("qqm:tr:{mid}"), "a track")
}

fn plugin_error(result: Result<impl std::fmt::Debug, CallError>) -> PluginError {
    match result {
        Err(CallError::Plugin(error)) => error,
        other => panic!("expected a plugin error, got {other:?}"),
    }
}

fn service_text(result: Result<impl std::fmt::Debug, CallError>) -> String {
    match plugin_error(result) {
        PluginError::Service(text) => text,
        other => panic!("expected a service error, got {other:?}"),
    }
}

#[test]
fn installs_only_as_a_community_plugin() {
    let env = Env::new();
    let wasm = component(dir(), PACKAGE, &[]);
    let files = production_files(dir());
    assert!(matches!(
        install_community(&env, &files, wasm.clone()),
        Err(HostError::CommunityDisabled)
    ));
    env.host.set_allow_community(true);
    let path = env.dir.path().join("qqmusic.jfpkg");
    std::fs::write(&path, files.package(wasm, false)).unwrap();
    let info = env.host.install(&path).unwrap();
    assert_eq!(info.id, ID);
    assert_eq!(info.name, "QQ Music (Unofficial)");
    assert!(!info.official);
    assert_eq!(info.signature, SignatureStatus::Unsigned);
    assert!(!info.enabled, "community plugins start turned off");
    assert_eq!(info.auth_kinds, vec![AuthKind::Qr]);
    assert_eq!(info.capabilities.len(), 6);
    assert_eq!(info.content_policy.library_storage, LibraryStorage::Metadata);
    assert_eq!(info.settings["quality"], "app");
    assert!(matches!(
        recv(env.host.search(ID, "mock", kinds(&[]), page(5))),
        Err(CallError::Disabled(_))
    ));
    env.host.set_enabled(ID, true).unwrap();
    let capabilities = recv(env.host.capabilities(ID)).unwrap();
    assert!(capabilities.contains(&Capability::Lyrics));
    assert!(matches!(
        env.host
            .set_settings(ID, &json!({ "mock_origin": "http://127.0.0.1:9" }).to_string()),
        Err(HostError::Settings(_))
    ));
    env.host.set_allow_community(false);
    assert!(matches!(
        recv(env.host.search(ID, "mock", kinds(&[]), page(5))),
        Err(CallError::CommunityDisabled)
    ));
}

#[test]
fn the_published_allowlist_is_six_hosts() {
    let manifest = jf_plugin_host::manifest::parse(&production_files(dir()).manifest).unwrap();
    assert_eq!(manifest.allow.len(), 6);
    for (host, allowed) in [
        ("u.y.qq.com", true),
        ("open.weixin.qq.com", true),
        ("lp.open.weixin.qq.com", true),
        ("ws.stream.qqmusic.qq.com", true),
        ("isure.stream.qqmusic.qq.com", true),
        ("y.gtimg.cn", true),
        ("qpic.y.qq.com", true),
        ("y.qq.com", false),
        ("c.y.qq.com", false),
        ("stream.qqmusic.qq.com", false),
        ("aqqmusic.tc.qq.com", false),
        ("ssl.ptlogin2.qq.com", false),
    ] {
        assert_eq!(
            manifest.allow.iter().any(|r| r.matches_fixed(host, 443)),
            allowed,
            "{host}"
        );
    }
    assert!(manifest.referenced_settings().is_empty());
    assert_eq!(manifest.max_rps, 5);
    assert!(!manifest.official_claim);
}

#[test]
fn wechat_sign_in() {
    let mock = Mock::start();
    let env = env_for(&mock);
    let flow = login(&env.host, ID, AuthKind::Qr);
    let AuthStep::ShowQr {
        data,
        hint,
        expires_at_ms,
    } = next_step(&flow)
    else {
        panic!("expected a QR code");
    };
    assert_eq!(
        data,
        "https://open.weixin.qq.com/connect/confirm?uuid=091MockUuid0001"
    );
    assert!(hint.unwrap().contains("WeChat"));
    assert!(expires_at_ms.is_some());
    // 408: still waiting.
    assert!(matches!(next_step(&flow), AuthStep::ShowQr { .. }));
    // 404: scanned.
    let AuthStep::Waiting { message } = next_step(&flow) else {
        panic!("expected a waiting step");
    };
    assert!(message.contains("Confirm"));
    // 405: confirmed; the code becomes a QQ Music session.
    assert_eq!(
        final_step(&flow),
        AuthStep::Done {
            account: "Mock QQ Listener".into()
        }
    );
    let stored = env
        .secrets
        .get(SECRET)
        .unwrap()
        .expect("the session is in the keystore");
    assert!(stored.contains("W_X_mock_key_0001") && stored.contains("7000001"));
    assert!(
        env.files_containing("W_X_mock_key_0001").is_empty(),
        "no key on disk"
    );
    let exchange = mock
        .calls()
        .into_iter()
        .find(|c| c.module == "music.login.LoginServer")
        .unwrap();
    assert_eq!(exchange.param["code"], "081MockWxCode0001");
    assert_eq!(exchange.param["strAppid"], mock::WECHAT_APP_ID);
    assert!(mock.violations().is_empty(), "{:?}", mock.violations());
}

#[test]
fn wechat_sign_in_cancelled_or_expired() {
    for (status, words) in [(403, "cancelled"), (402, "expired")] {
        let mock = Mock::with(|state| state.wechat_script = vec![408, status]);
        let env = env_for(&mock);
        let flow = login(&env.host, ID, AuthKind::Qr);
        assert!(matches!(next_step(&flow), AuthStep::ShowQr { .. }));
        let AuthStep::Failed { message } = final_step(&flow) else {
            panic!("expected a failure");
        };
        assert!(message.contains(words), "{message}");
        assert!(env.secrets.get(SECRET).unwrap().is_none());
    }
}

#[test]
fn every_request_is_signed() {
    let mock = quick_mock();
    let env = env_for(&mock);
    recv(env.host.search(ID, "mock", kinds(&[]), page(10))).unwrap();
    sign_in(&env);
    recv(
        env.host
            .browse(ID, Some(item(ItemKind::Folder, "qqm:f:liked", "Liked")), page(10)),
    )
    .unwrap();
    recv(
        env.host
            .resolve_stream(ID, track("0000MockSong01"), Quality::High),
    )
    .unwrap();
    let calls = mock.calls();
    assert!(calls.len() >= 9, "{}", calls.len());
    assert!(mock.violations().is_empty(), "{:?}", mock.violations());
    // Signed out: no key; signed in: the key in comm and in the cookies.
    assert_eq!(calls[0].comm["uin"], 0);
    assert_eq!(calls[0].comm["g_tk"], 5381);
    let last = calls.last().unwrap();
    assert_eq!(last.comm["authst"], "W_X_mock_key_0001");
    assert_eq!(last.comm["tmeLoginType"], 1);
    assert!(last.cookies.contains("qm_keyst=W_X_mock_key_0001"));
}

#[test]
fn search_lists_every_kind() {
    let mock = quick_mock();
    let env = env_for(&mock);
    let found = recv(env.host.search(ID, "mock", kinds(&[]), page(20))).unwrap();
    let get = |id: &str| {
        found
            .items
            .iter()
            .find(|i| i.id == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    assert_eq!(get("qqm:ar:0000MockSinger").title, "Mock Singer");
    let album = get("qqm:al:0000MockAlbum1");
    assert_eq!((album.year, album.child_count), (Some(2020), Some(3)));
    assert_eq!(
        get("qqm:pl:8000003").artist.as_deref(),
        Some("Someone Else (mock)")
    );
    let one = get("qqm:tr:0000MockSong01");
    assert!(one.playable);
    assert_eq!(one.isrc.as_deref(), Some("CNZZ02000001"));
    assert_eq!(one.duration_ms, Some(200_000));
    assert_eq!(
        one.artwork.as_ref().unwrap().url,
        format!(
            "{}/y.gtimg.cn/music/photo_new/T002R300x300M0000000MockAlbum1.jpg?max_age=2592000",
            mock.server.url()
        )
    );
    assert_eq!(
        get("qqm:tr:0000MockSong02").artist.as_deref(),
        Some("Mock Singer, Guest Singer")
    );
    let silent = get("qqm:tr:0000MockSong05");
    assert!(!silent.playable);
    assert_eq!(
        silent.unplayable.as_ref().unwrap().reason,
        UnplayableReason::Unavailable
    );

    // Only tracks, three at a time: the service pages by number.
    let mut cursor = None;
    let mut seen = Vec::new();
    loop {
        let found = recv(
            env.host
                .search(ID, "mock song", kinds(&[ItemKind::Track]), next_page(cursor, 3)),
        )
        .unwrap();
        assert!(found.items.len() <= 3);
        seen.extend(ids(&found));
        cursor = found.next;
        if cursor.is_none() {
            break;
        }
    }
    let expected: Vec<String> = (1..=7).map(|n| format!("qqm:tr:0000MockSong0{n}")).collect();
    assert_eq!(seen, expected);
    let pages: Vec<u64> = mock
        .calls()
        .iter()
        .filter(|c| c.param["query"] == "mock song")
        .map(|c| c.param["page_num"].as_u64().unwrap())
        .collect();
    assert_eq!(pages, [1, 2, 3]);
    assert!(mock.violations().is_empty(), "{:?}", mock.violations());
}

#[test]
fn the_library_needs_a_sign_in() {
    let mock = quick_mock();
    let env = env_for(&mock);
    assert_eq!(
        plugin_error(recv(env.host.browse(ID, None, page(10)))),
        PluginError::AuthRequired
    );
    sign_in(&env);
    let root = recv(env.host.browse(ID, None, page(10))).unwrap();
    assert_eq!(ids(&root), ["qqm:f:liked", "qqm:f:playlists"]);
    let liked = recv(env.host.browse(ID, Some(root.items[0].clone()), page(10))).unwrap();
    assert_eq!(
        ids(&liked),
        [
            "qqm:tr:0000MockSong02",
            "qqm:tr:0000MockSong01",
            "qqm:tr:0000MockSong06"
        ]
    );
    assert_eq!(liked.total, Some(3));
    let own = recv(env.host.browse(ID, Some(root.items[1].clone()), page(10))).unwrap();
    assert_eq!(
        ids(&own),
        ["qqm:pl:8000002"],
        "the liked songs have their own section"
    );
    assert_eq!(
        mock.count("music.musicasset.PlaylistBaseRead", "GetPlaylistByUin"),
        1,
        "the account's playlists are asked for once"
    );
    assert_eq!(own.items[0].title, "Mock Commute");
    assert_eq!(own.items[0].child_count, Some(7));

    // The service pages a playlist: three songs a page.
    mock.clear();
    let mut cursor = None;
    let mut seen = Vec::new();
    loop {
        let found = recv(
            env.host
                .browse(ID, Some(own.items[0].clone()), next_page(cursor, 3)),
        )
        .unwrap();
        assert_eq!(found.total, Some(7));
        seen.extend(ids(&found));
        cursor = found.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen.len(), 7);
    let begins: Vec<u64> = mock
        .calls()
        .iter()
        .map(|c| c.param["song_begin"].as_u64().unwrap())
        .collect();
    assert_eq!(begins, [0, 3, 6]);
    assert!(mock.violations().is_empty(), "{:?}", mock.violations());
}

#[test]
fn albums_and_singers() {
    let mock = quick_mock();
    let env = env_for(&mock);
    let album = recv(env.host.browse(
        ID,
        Some(item(ItemKind::Album, "qqm:al:0000MockAlbum1", "x")),
        page(2),
    ))
    .unwrap();
    assert_eq!(ids(&album), ["qqm:tr:0000MockSong01", "qqm:tr:0000MockSong02"]);
    assert_eq!((album.total, album.next.as_deref()), (Some(3), Some("2")));
    let rest = recv(env.host.browse(
        ID,
        Some(item(ItemKind::Album, "qqm:al:0000MockAlbum1", "x")),
        next_page(album.next.clone(), 2),
    ))
    .unwrap();
    assert_eq!(ids(&rest), ["qqm:tr:0000MockSong06"]);
    assert_eq!(rest.next, None);
    let albums = recv(env.host.browse(
        ID,
        Some(item(ItemKind::Artist, "qqm:ar:0000MockSinger", "x")),
        page(10),
    ))
    .unwrap();
    assert_eq!(ids(&albums), ["qqm:al:0000MockAlbum1", "qqm:al:0000MockAlbum2"]);
    assert_eq!(albums.items[1].year, Some(2021));
    assert!(matches!(
        plugin_error(recv(env.host.browse(
            ID,
            Some(item(ItemKind::Album, "qqm:al:0000NoSuchAlb", "x")),
            page(5)
        ))),
        PluginError::Service(_)
    ));
}

#[test]
fn streams_whole_tracks_only() {
    let mock = quick_mock();
    let env = env_for(&mock);
    assert_eq!(
        plugin_error(recv(env.host.resolve_stream(
            ID,
            track("0000MockSong01"),
            Quality::High
        ))),
        PluginError::AuthRequired
    );
    sign_in(&env);
    let base = mock.server.url();

    let high = recv(
        env.host
            .resolve_stream(ID, track("0000MockSong01"), Quality::High),
    )
    .unwrap();
    assert!(
        high.url.starts_with(&format!(
            "{base}/ws.stream.qqmusic.qq.com/M8000000MockMedia1.mp3?guid="
        )),
        "{}",
        high.url
    );
    assert!(high.url.contains("vkey=MOCKVKEYM800"));
    assert_eq!(
        (high.codec.as_deref(), high.bitrate_kbps),
        (Some("mp3"), Some(320))
    );
    assert!(!high.cacheable && high.expires_at_ms.is_some());
    let fetched = fetch(&high.url, &[]).unwrap();
    assert_eq!((fetched.status, fetched.body.as_slice()), (200, AUDIO));

    // Only a 128 kbit/s file exists: that one, not nothing.
    let lower = recv(
        env.host
            .resolve_stream(ID, track("0000MockSong02"), Quality::High),
    )
    .unwrap();
    assert!(lower.url.contains("/M5000000MockMedia2.mp3?"));
    assert_eq!(lower.bitrate_kbps, Some(128));
    let lossless = recv(
        env.host
            .resolve_stream(ID, track("0000MockSong06"), Quality::Lossless),
    )
    .unwrap();
    assert!(lossless.url.contains("/F0000000MockMedia6.flac?"));
    assert_eq!(
        (lossless.codec.as_deref(), lossless.bitrate_kbps),
        (Some("flac"), None)
    );
    let low = recv(env.host.resolve_stream(ID, track("0000MockSong06"), Quality::Low)).unwrap();
    assert_eq!((low.codec.as_deref(), low.bitrate_kbps), (Some("aac"), Some(96)));

    // VIP-only: nothing is granted, and the answer's preview file is ignored.
    let text = service_text(recv(env.host.resolve_stream(
        ID,
        track("0000MockSong03"),
        Quality::High,
    )));
    assert!(text.contains("VIP"), "{text}");
    // A path for another file than the one asked for is a preview: refused.
    let text = service_text(recv(env.host.resolve_stream(
        ID,
        track("0000MockSong04"),
        Quality::High,
    )));
    assert!(
        text.contains("preview") && text.contains("whole tracks only"),
        "{text}"
    );
    assert!(
        service_text(recv(env.host.resolve_stream(
            ID,
            track("0000MockSong05"),
            Quality::High
        )))
        .contains("no audio")
    );
    let text = service_text(recv(env.host.resolve_stream(
        ID,
        track("0000MockSong07"),
        Quality::High,
    )));
    assert!(text.contains("code 104003"), "{text}");
    assert!(matches!(
        plugin_error(recv(env.host.resolve_stream(
            ID,
            item(ItemKind::Track, "tr:1", "x"),
            Quality::High
        ))),
        PluginError::InvalidInput(_)
    ));

    // Later listings say what the service refused.
    let members = recv(env.host.browse(
        ID,
        Some(item(ItemKind::Album, "qqm:al:0000MockAlbum2", "x")),
        page(10),
    ))
    .unwrap();
    for listed in &members.items {
        assert!(!listed.playable, "{}", listed.id);
        assert_eq!(
            listed.unplayable.as_ref().unwrap().reason,
            UnplayableReason::Paywall
        );
    }
    let served = mock.state.lock().unwrap().served.clone();
    assert!(served.iter().all(|p| !p.contains("RS02")), "{served:?}");
    assert!(mock.violations().is_empty(), "{:?}", mock.violations());
}

#[test]
fn the_quality_setting_wins() {
    let mock = quick_mock();
    let (env, info) = with_mock(dir(), PACKAGE, &mock.server, json!({ "quality": "normal" }));
    sign_in(&env);
    let stream = recv(
        env.host
            .resolve_stream(&info.id, track("0000MockSong06"), Quality::Lossless),
    )
    .unwrap();
    assert_eq!(stream.bitrate_kbps, Some(128));
    let asked = mock
        .calls()
        .into_iter()
        .find(|c| c.module == "vkey.GetVkeyServer")
        .unwrap();
    assert_eq!(
        asked.param["filename"],
        json!(["M5000000MockMedia6.mp3", "C4000000MockMedia6.m4a"])
    );
}

#[test]
fn lyrics() {
    let mock = quick_mock();
    let env = env_for(&mock);
    let synced = recv(env.host.lyrics(ID, track("0000MockSong01")))
        .unwrap()
        .expect("lyrics");
    assert_eq!(synced.format, LyricFormat::Lrc);
    assert_eq!(synced.text, include_str!("golden/song01-translated.lrc"));
    assert_eq!(synced.attribution.as_deref(), Some("Lyrics from QQ Music"));
    assert_eq!(
        recv(env.host.lyrics(ID, track("0000MockSong02"))).unwrap(),
        None,
        "an instrumental"
    );
    assert_eq!(
        recv(env.host.lyrics(ID, track("0000MockSong06"))).unwrap(),
        None,
        "encrypted"
    );
    assert_eq!(
        recv(env.host.lyrics(ID, track("0000MockSong07"))).unwrap(),
        None,
        "none"
    );
    let mut foreign = item(ItemKind::Track, "local:1", "Mock Song One");
    foreign.artist = Some("Mock Singer".into());
    foreign.duration_ms = Some(201_000);
    let found = recv(env.host.lyrics(ID, foreign.clone()))
        .unwrap()
        .expect("lyrics");
    assert_eq!(found.text, include_str!("golden/song01-translated.lrc"));
    foreign.artist = Some("Somebody Else".into());
    assert_eq!(
        recv(env.host.lyrics(ID, foreign)).unwrap(),
        None,
        "another artist"
    );
    assert!(mock.violations().is_empty(), "{:?}", mock.violations());
}

#[test]
fn lyrics_without_translations() {
    let mock = quick_mock();
    let (env, info) = with_mock(dir(), PACKAGE, &mock.server, json!({ "translations": false }));
    let lyrics = recv(env.host.lyrics(&info.id, track("0000MockSong01")))
        .unwrap()
        .unwrap();
    assert_eq!(lyrics.text, include_str!("golden/song01.lrc"));
}

#[test]
fn artwork() {
    let mock = quick_mock();
    let env = env_for(&mock);
    let base = mock.server.url();
    let album = recv(
        env.host
            .artwork(ID, item(ItemKind::Album, "qqm:al:0000MockAlbum2", "x"), 400),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        album.url,
        format!("{base}/y.gtimg.cn/music/photo_new/T002R500x500M0000000MockAlbum2.jpg?max_age=2592000")
    );
    assert_eq!(album.width, Some(500));
    let fetched = fetch(&album.url, &[]).unwrap();
    assert_eq!((fetched.status, fetched.body.as_slice()), (200, PICTURE));
    let singer = recv(
        env.host
            .artwork(ID, item(ItemKind::Artist, "qqm:ar:0000MockSinger", "x"), 90),
    )
    .unwrap()
    .unwrap();
    assert!(singer.url.contains("/T001R90x90M0000000MockSinger.jpg"));
    // A track: from the album id it carries, else from the song's details.
    let mut with_album = track("0000MockSong01");
    with_album.album_id = Some("qqm:al:0000MockAlbum1".into());
    mock.clear();
    let art = recv(env.host.artwork(ID, with_album, 300)).unwrap().unwrap();
    assert!(art.url.contains("M0000000MockAlbum1.jpg"));
    assert!(mock.calls().is_empty(), "no request needed");
    let art = recv(env.host.artwork(ID, track("0000MockSong03"), 300))
        .unwrap()
        .unwrap();
    assert!(art.url.contains("M0000000MockAlbum2.jpg"));
    let cover = recv(
        env.host
            .artwork(ID, item(ItemKind::Playlist, "qqm:pl:8000003", "x"), 300),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        cover.url,
        format!("{base}/qpic.y.qq.com/music_cover/mock-public/300")
    );
    // Items from other sources, found by search.
    let mut foreign = item(ItemKind::Track, "local:2", "Mock Song Two");
    foreign.artist = Some("Mock Singer".into());
    assert!(
        recv(env.host.artwork(ID, foreign, 300))
            .unwrap()
            .unwrap()
            .url
            .contains("0000MockAlbum1")
    );
    let mut foreign_album = item(ItemKind::Album, "local:al", "Mock Members Album");
    foreign_album.artist = Some("Mock Singer".into());
    assert!(
        recv(env.host.artwork(ID, foreign_album, 300))
            .unwrap()
            .unwrap()
            .url
            .contains("0000MockAlbum2")
    );
    assert_eq!(
        recv(
            env.host
                .artwork(ID, item(ItemKind::Track, "local:3", "No Such Song"), 300)
        )
        .unwrap(),
        None
    );
    assert!(mock.violations().is_empty(), "{:?}", mock.violations());
}

#[test]
fn a_session_key_is_renewed_before_it_runs_out() {
    let mock = Mock::with(|state| {
        state.wechat_script = vec![405];
        state.key_lifetime_s = 1;
    });
    let env = env_for(&mock);
    sign_in(&env);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    mock.expire_keys();
    let liked = recv(
        env.host
            .browse(ID, Some(item(ItemKind::Folder, "qqm:f:liked", "Liked")), page(10)),
    )
    .unwrap();
    assert_eq!(liked.items.len(), 3);
    let renewals: Vec<_> = mock
        .calls()
        .into_iter()
        .filter(|c| c.module == "music.login.LoginServer" && c.param.get("refresh_key").is_some())
        .collect();
    assert_eq!(renewals.len(), 1);
    assert_eq!(renewals[0].param["refresh_key"], "mock_refresh_0001");
    let stored = env.secrets.get(SECRET).unwrap().unwrap();
    assert!(stored.contains("W_X_mock_key_0002"), "the renewed key is kept");
    assert!(mock.violations().is_empty(), "{:?}", mock.violations());
}

#[test]
fn an_expired_session_asks_to_sign_in_again() {
    let mock = Mock::with(|state| {
        state.wechat_script = vec![405];
        state.refresh_works = false;
    });
    let env = env_for(&mock);
    sign_in(&env);
    mock.expire_keys();
    assert_eq!(
        plugin_error(recv(env.host.browse(
            ID,
            Some(item(ItemKind::Folder, "qqm:f:playlists", "x")),
            page(10)
        ))),
        PluginError::AuthRequired
    );
}

#[test]
fn signing_out_forgets_the_session() {
    let mock = quick_mock();
    let env = env_for(&mock);
    sign_in(&env);
    recv(env.host.logout(ID)).unwrap();
    assert!(env.secrets.get(SECRET).unwrap().is_none());
    assert_eq!(env.host.plugin(ID).unwrap().account, None);
    assert_eq!(
        plugin_error(recv(env.host.browse(ID, None, page(10)))),
        PluginError::AuthRequired
    );
}
