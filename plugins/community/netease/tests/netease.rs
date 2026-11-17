//! The NetEase Cloud Music plugin: the real component in the real Jellyfesh
//! plugin host, installed unsigned as a community plugin, against a mock of
//! the weapi service (`mock/mod.rs`, invented data in `fixtures/`).
//!
//! The live service was never reachable from where these tests were written:
//! they show the plugin speaks the protocol as its author understands it,
//! not that NetEase answers that way today (see the README).

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

const ID: &str = "community.unofficial.netease";
const PACKAGE: &str = "jf-community-netease";
const SECRET: &str = "jellyfesh/community.unofficial.netease/session";

fn dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// A host with the plugin pointed at the mock.
fn env_for(mock: &Mock) -> Env {
    with_mock(dir(), PACKAGE, &mock.server, json!({})).0
}

/// A mock whose QR sign-in succeeds on the first poll.
fn quick_mock() -> Mock {
    Mock::with(|state| state.qr_script = vec![803])
}

/// Signs in through the QR flow (the mock confirms on its first poll).
fn sign_in(env: &Env) {
    let flow = login(&env.host, ID, AuthKind::Qr);
    assert!(matches!(next_step(&flow), AuthStep::ShowQr { .. }));
    assert_eq!(
        final_step(&flow),
        AuthStep::Done {
            account: "Mock Listener".into()
        }
    );
}

fn ids(page: &jf_plugin_host::ItemPage) -> Vec<String> {
    page.items.iter().map(|i| i.id.clone()).collect()
}

fn track(id: u64) -> Item {
    item(ItemKind::Track, &format!("ncm:tr:{id}"), "a track")
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
    // Community plugins are off by default: the unsigned package is refused.
    assert!(matches!(
        install_community(&env, &files, wasm.clone()),
        Err(HostError::CommunityDisabled)
    ));
    env.host.set_allow_community(true);
    let bytes = files.package(wasm, false);
    let path = env.dir.path().join("netease.jfpkg");
    std::fs::write(&path, bytes).unwrap();
    let info = env.host.install(&path).unwrap();
    assert_eq!(info.id, ID);
    assert_eq!(info.name, "NetEase Cloud Music (Unofficial)");
    assert!(!info.official);
    assert_eq!(info.signature, SignatureStatus::Unsigned);
    assert!(!info.enabled, "community plugins start turned off");
    assert_eq!(info.auth_kinds, vec![AuthKind::Qr]);
    assert_eq!(info.capabilities.len(), 6);
    assert_eq!(info.content_policy.library_storage, LibraryStorage::Metadata);
    assert_eq!(info.settings["quality"], "app");
    assert_eq!(info.settings["translations"], true);
    assert!(matches!(
        recv(env.host.search(ID, "mock", kinds(&[]), page(5))),
        Err(CallError::Disabled(_))
    ));
    env.host.set_enabled(ID, true).unwrap();
    let capabilities = recv(env.host.capabilities(ID)).unwrap();
    assert!(capabilities.contains(&Capability::ResolveStream));
    assert_eq!(capabilities.len(), 6);
    // The published package has no way to point it anywhere else.
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
fn the_published_allowlist_is_two_hosts() {
    let manifest = jf_plugin_host::manifest::parse(&production_files(dir()).manifest).unwrap();
    assert_eq!(manifest.allow.len(), 2);
    assert!(manifest.allow[0].matches_fixed("music.163.com", 443));
    assert!(manifest.allow[1].matches_fixed("m701.music.126.net", 443));
    assert!(manifest.allow[1].matches_fixed("p1.music.126.net", 443));
    assert!(
        !manifest
            .allow
            .iter()
            .any(|r| r.matches_fixed("music.126.net", 443))
    );
    assert!(
        !manifest
            .allow
            .iter()
            .any(|r| r.matches_fixed("interface.music.163.com", 443))
    );
    assert!(manifest.referenced_settings().is_empty());
    assert_eq!(manifest.max_rps, 5);
    assert!(!manifest.official_claim);
    assert!(!manifest.id.starts_with("org.jellyfesh."));
}

#[test]
fn qr_sign_in() {
    let mock = Mock::start();
    let env = env_for(&mock);
    let flow = login(&env.host, ID, AuthKind::Qr);
    let AuthStep::ShowQr { data, hint, .. } = next_step(&flow) else {
        panic!("expected a QR code");
    };
    assert_eq!(data, "https://music.163.com/login?codekey=mock-unikey-1");
    assert!(hint.unwrap().contains("NetEase Cloud Music app"));
    // 801: still waiting for a scan.
    assert!(matches!(next_step(&flow), AuthStep::ShowQr { .. }));
    // 802: scanned, waiting for the confirmation.
    let AuthStep::Waiting { message } = next_step(&flow) else {
        panic!("expected a waiting step");
    };
    assert!(message.contains("Mock Listener") && message.contains("Confirm"));
    // 803: signed in.
    assert_eq!(
        final_step(&flow),
        AuthStep::Done {
            account: "Mock Listener".into()
        }
    );
    assert_eq!(
        env.host.plugin(ID).unwrap().account.as_deref(),
        Some("Mock Listener")
    );
    let stored = env
        .secrets
        .get(SECRET)
        .unwrap()
        .expect("the session is in the keystore");
    assert!(stored.contains("90001") && stored.contains("mockcsrf0001"));
    // The session cookie stays in the host's jar, which is encrypted on disk.
    assert!(env.files_containing("mock-music-u-1").is_empty());
    assert!(env.files_containing("mockcsrf0001").is_empty());

    mock.clear();
    recv(env.host.browse(
        ID,
        Some(item(ItemKind::Folder, "ncm:f:liked", "Liked Songs")),
        page(10),
    ))
    .unwrap();
    for request in mock.decoded() {
        assert_eq!(
            request.params["csrf_token"], "mockcsrf0001",
            "{}",
            request.endpoint
        );
        assert!(
            request.cookies.contains("MUSIC_U=mock-music-u-1"),
            "{}",
            request.endpoint
        );
    }
    assert!(mock.violations().is_empty(), "{:?}", mock.violations());
}

#[test]
fn qr_code_expires() {
    let mock = Mock::with(|state| state.qr_script = vec![801, 800]);
    let env = env_for(&mock);
    let flow = login(&env.host, ID, AuthKind::Qr);
    assert!(matches!(next_step(&flow), AuthStep::ShowQr { .. }));
    let AuthStep::Failed { message } = final_step(&flow) else {
        panic!("expected a failure");
    };
    assert!(message.contains("expired"), "{message}");
    assert!(env.secrets.get(SECRET).unwrap().is_none());
}

#[test]
fn every_request_is_weapi_encoded() {
    let mock = quick_mock();
    let env = env_for(&mock);
    recv(env.host.search(ID, "mock", kinds(&[]), page(10))).unwrap();
    sign_in(&env);
    recv(env.host.browse(ID, None, page(10))).unwrap();
    recv(env.host.resolve_stream(ID, track(1001), Quality::High)).unwrap();
    let decoded = mock.decoded();
    assert!(decoded.len() >= 7, "{}", decoded.len());
    assert!(mock.violations().is_empty(), "{:?}", mock.violations());
    // A fresh random key for every request.
    let mut keys: Vec<&str> = decoded.iter().map(|d| d.key.as_str()).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), decoded.len());
    // Signed out, the token is empty; signed in, it is the session's.
    assert_eq!(decoded[0].params["csrf_token"], "");
    assert_eq!(decoded.last().unwrap().query_csrf, "mockcsrf0001");
}

#[test]
fn search_lists_every_kind_and_what_cannot_play() {
    let mock = quick_mock();
    let env = env_for(&mock);
    // Search works without signing in.
    let found = recv(env.host.search(ID, "mock", kinds(&[]), page(20))).unwrap();
    let kinds_found: Vec<ItemKind> = found.items.iter().map(|i| i.kind).collect();
    for kind in [
        ItemKind::Artist,
        ItemKind::Album,
        ItemKind::Playlist,
        ItemKind::Track,
    ] {
        assert!(kinds_found.contains(&kind), "{kind:?}");
    }
    // A tenth of the page each for artists, albums and playlists: the third
    // playlist comes on the next page, after everything else ran out.
    assert_eq!(found.next.as_deref(), Some("-,-,2,-"));
    let rest = recv(
        env.host
            .search(ID, "mock", kinds(&[]), next_page(found.next.clone(), 20)),
    )
    .unwrap();
    assert_eq!(ids(&rest), ["ncm:pl:3003"]);
    assert_eq!(rest.next, None);
    let get = |id: &str| {
        found
            .items
            .iter()
            .find(|i| i.id == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    let one = get("ncm:tr:1001");
    assert!(one.playable);
    assert_eq!(one.artist.as_deref(), Some("Mock Artist"));
    assert_eq!(one.album_id.as_deref(), Some("ncm:al:701"));
    assert_eq!(one.duration_ms, Some(200_000));
    assert_eq!(one.year, Some(2020));
    let art = one.artwork.as_ref().expect("cover art");
    assert_eq!(
        art.url,
        format!(
            "{}/p1.music.126.net/mock/album-701.jpg?param=300y300",
            mock.server.url()
        )
    );
    assert_eq!(
        get("ncm:tr:1002").artist.as_deref(),
        Some("Mock Artist, Guest Mock")
    );
    let vip = get("ncm:tr:1003");
    assert!(!vip.playable);
    let reason = vip.unplayable.as_ref().unwrap();
    assert_eq!(reason.reason, UnplayableReason::Paywall);
    assert!(reason.message.as_deref().unwrap().contains("VIP"));
    assert_eq!(
        get("ncm:tr:1004").unplayable.as_ref().unwrap().reason,
        UnplayableReason::Paywall
    );
    assert_eq!(
        get("ncm:tr:1005").unplayable.as_ref().unwrap().reason,
        UnplayableReason::Unavailable
    );
    assert_eq!(get("ncm:ar:501").title, "Mock Artist");
    assert_eq!(get("ncm:al:701").child_count, Some(3));
    assert_eq!(get("ncm:pl:3002").child_count, Some(7));

    // Only tracks, three at a time.
    let first = recv(env.host.search(ID, "mock", kinds(&[ItemKind::Track]), page(3))).unwrap();
    assert_eq!(ids(&first), ["ncm:tr:1001", "ncm:tr:1002", "ncm:tr:1003"]);
    let second = recv(env.host.search(
        ID,
        "mock",
        kinds(&[ItemKind::Track]),
        next_page(first.next.clone(), 3),
    ))
    .unwrap();
    assert_eq!(ids(&second), ["ncm:tr:1004", "ncm:tr:1005", "ncm:tr:1006"]);
    let third = recv(env.host.search(
        ID,
        "mock",
        kinds(&[ItemKind::Track]),
        next_page(second.next.clone(), 3),
    ))
    .unwrap();
    assert_eq!(ids(&third), ["ncm:tr:1007", "ncm:tr:1008"]);
    assert_eq!(third.next, None);
    assert!(mock.violations().is_empty(), "{:?}", mock.violations());
}

#[test]
fn scrambled_search_results_are_an_error() {
    let mock = Mock::with(|state| state.scrambled_search = true);
    let env = env_for(&mock);
    let text = service_text(recv(env.host.search(ID, "mock", kinds(&[]), page(10))));
    assert!(text.contains("scrambled"), "{text}");
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
    assert_eq!(ids(&root), ["ncm:f:liked", "ncm:f:playlists", "ncm:f:saved"]);

    let liked = recv(env.host.browse(ID, Some(root.items[0].clone()), page(10))).unwrap();
    assert_eq!(ids(&liked), ["ncm:tr:1002", "ncm:tr:1001", "ncm:tr:1007"]);
    assert_eq!(liked.total, Some(3));
    let own = recv(env.host.browse(ID, Some(root.items[1].clone()), page(10))).unwrap();
    assert_eq!(
        ids(&own),
        ["ncm:pl:3002"],
        "the liked songs have their own section"
    );
    let saved = recv(env.host.browse(ID, Some(root.items[2].clone()), page(10))).unwrap();
    assert_eq!(ids(&saved), ["ncm:pl:3003"]);
    assert_eq!(saved.items[0].artist.as_deref(), Some("Someone Else (mock)"));

    // A playlist, three tracks a page: its ids once, then each page's songs.
    mock.clear();
    let list = own.items[0].clone();
    let mut cursor = None;
    let mut seen = Vec::new();
    loop {
        let page = recv(env.host.browse(ID, Some(list.clone()), next_page(cursor, 3))).unwrap();
        assert_eq!(page.total, Some(7));
        seen.extend(ids(&page));
        cursor = page.next.clone();
        if cursor.is_none() {
            break;
        }
    }
    let expected: Vec<String> = (1001..=1007).map(|id| format!("ncm:tr:{id}")).collect();
    assert_eq!(seen, expected, "in the playlist's order");
    assert_eq!(mock.calls("/v6/playlist/detail"), 1);
    assert_eq!(mock.calls("/v3/song/detail"), 3);
    assert!(mock.violations().is_empty(), "{:?}", mock.violations());
}

#[test]
fn albums_and_artists() {
    let mock = quick_mock();
    let env = env_for(&mock);
    sign_in(&env);
    let album = recv(env.host.browse(
        ID,
        Some(item(ItemKind::Album, "ncm:al:701", "Mock Album")),
        page(50),
    ))
    .unwrap();
    assert_eq!(ids(&album), ["ncm:tr:1001", "ncm:tr:1002", "ncm:tr:1007"]);
    assert_eq!(
        album.items.iter().map(|i| i.track_number).collect::<Vec<_>>(),
        [Some(1), Some(2), Some(3)]
    );
    // The album's songs came without privileges, so the plugin asked for them.
    assert_eq!(mock.calls("/v3/song/detail"), 1);
    let members = recv(env.host.browse(
        ID,
        Some(item(ItemKind::Album, "ncm:al:702", "Mock Members Album")),
        page(50),
    ))
    .unwrap();
    assert!(members.items.iter().all(|i| !i.playable));
    assert!(
        members
            .items
            .iter()
            .all(|i| i.unplayable.as_ref().unwrap().reason == UnplayableReason::Paywall)
    );
    let artist = recv(env.host.browse(
        ID,
        Some(item(ItemKind::Artist, "ncm:ar:501", "Mock Artist")),
        page(1),
    ))
    .unwrap();
    assert_eq!(ids(&artist), ["ncm:al:701"]);
    let rest = recv(env.host.browse(
        ID,
        Some(item(ItemKind::Artist, "ncm:ar:501", "Mock Artist")),
        next_page(artist.next.clone(), 1),
    ))
    .unwrap();
    assert_eq!(ids(&rest), ["ncm:al:702"]);
    assert_eq!(rest.next, None);
    assert_eq!(
        plugin_error(recv(env.host.browse(
            ID,
            Some(item(ItemKind::Album, "ncm:al:9", "x")),
            page(5)
        ))),
        PluginError::NotFound
    );
}

#[test]
fn streams_whole_tracks_only() {
    let mock = quick_mock();
    let env = env_for(&mock);
    assert_eq!(
        plugin_error(recv(env.host.resolve_stream(ID, track(1001), Quality::High))),
        PluginError::AuthRequired
    );
    sign_in(&env);

    let stream = recv(env.host.resolve_stream(ID, track(1001), Quality::High)).unwrap();
    assert_eq!(
        stream.url,
        format!(
            "{}/m701.music.126.net/mock/1001-320000.mp3?vuutv=mock",
            mock.server.url()
        )
    );
    assert_eq!(stream.codec.as_deref(), Some("mp3"));
    assert_eq!(stream.bitrate_kbps, Some(320));
    assert!(!stream.cacheable);
    assert!(stream.expires_at_ms.is_some());
    let fetched = fetch(&stream.url, &[]).unwrap();
    assert_eq!((fetched.status, fetched.body.as_slice()), (200, AUDIO));

    let lossless = recv(env.host.resolve_stream(ID, track(1007), Quality::Lossless)).unwrap();
    assert_eq!(
        (lossless.codec.as_deref(), lossless.bitrate_kbps),
        (Some("flac"), Some(999))
    );
    let low = recv(env.host.resolve_stream(ID, track(1007), Quality::Low)).unwrap();
    assert_eq!((low.codec.as_deref(), low.bitrate_kbps), (Some("mp3"), Some(128)));

    // A VIP song: the service offers only a marked preview, which is refused.
    let text = service_text(recv(env.host.resolve_stream(ID, track(1003), Quality::High)));
    assert!(
        text.contains("preview") && text.contains("whole tracks only"),
        "{text}"
    );
    // A clip without the mark, but far shorter than the song: refused too,
    // whether the item says how long the song is or not.
    let mut clipped = track(1006);
    clipped.duration_ms = Some(210_000);
    assert!(service_text(recv(env.host.resolve_stream(ID, clipped, Quality::High))).contains("preview"));
    assert!(service_text(recv(env.host.resolve_stream(ID, track(1006), Quality::High))).contains("preview"));
    // No stream at all.
    assert!(service_text(recv(env.host.resolve_stream(ID, track(1004), Quality::High))).contains("bought"));
    assert!(
        service_text(recv(env.host.resolve_stream(ID, track(1005), Quality::High))).contains("no stream")
    );
    // A stream on a host outside music.126.net is not handed out.
    assert!(
        service_text(recv(env.host.resolve_stream(ID, track(1008), Quality::High)))
            .contains("does not trust")
    );
    // Other sources' tracks are not this plugin's to play.
    assert!(matches!(
        plugin_error(recv(env.host.resolve_stream(
            ID,
            item(ItemKind::Track, "tr:1001", "x"),
            Quality::High
        ))),
        PluginError::InvalidInput(_)
    ));
    let served = mock.state.lock().unwrap().served.clone();
    assert!(
        served
            .iter()
            .all(|path| !path.contains("preview") && !path.contains("clip")),
        "{served:?}"
    );
    assert!(mock.violations().is_empty(), "{:?}", mock.violations());
}

#[test]
fn the_quality_setting_wins() {
    let mock = quick_mock();
    let (env, info) = with_mock(dir(), PACKAGE, &mock.server, json!({ "quality": "low" }));
    sign_in(&env);
    let stream = recv(env.host.resolve_stream(&info.id, track(1007), Quality::Lossless)).unwrap();
    assert_eq!(stream.bitrate_kbps, Some(128));
    let asked = mock
        .decoded()
        .into_iter()
        .find(|d| d.endpoint == "/song/enhance/player/url/v1")
        .unwrap();
    assert_eq!(asked.params["level"], "standard");
}

#[test]
fn falls_back_to_the_older_stream_endpoint() {
    let mock = Mock::with(|state| {
        state.qr_script = vec![803];
        state.v1_fails = true;
    });
    let env = env_for(&mock);
    sign_in(&env);
    let stream = recv(env.host.resolve_stream(ID, track(1001), Quality::Normal)).unwrap();
    assert_eq!(stream.bitrate_kbps, Some(192));
    assert_eq!(mock.calls("/song/enhance/player/url/v1"), 1);
    assert_eq!(mock.calls("/song/enhance/player/url"), 1);
    let legacy = mock
        .decoded()
        .into_iter()
        .find(|d| d.endpoint == "/song/enhance/player/url")
        .unwrap();
    assert_eq!(legacy.params["br"], 192_000);
    assert_eq!(legacy.params["ids"], "[1001]");
}

#[test]
fn lyrics() {
    let mock = quick_mock();
    let env = env_for(&mock);
    let synced = recv(env.host.lyrics(ID, track(1001))).unwrap().expect("lyrics");
    assert_eq!(synced.format, LyricFormat::Lrc);
    assert_eq!(synced.text, include_str!("golden/1001-translated.lrc"));
    assert_eq!(
        synced.attribution.as_deref(),
        Some("Lyrics from NetEase Cloud Music")
    );
    let plain = recv(env.host.lyrics(ID, track(1007))).unwrap().expect("lyrics");
    assert_eq!(plain.format, LyricFormat::Plain);
    assert_eq!(plain.text, include_str!("golden/1007.txt"));
    assert_eq!(recv(env.host.lyrics(ID, track(1002))).unwrap(), None, "no lyrics");
    assert_eq!(
        recv(env.host.lyrics(ID, track(1006))).unwrap(),
        None,
        "an instrumental"
    );
    assert_eq!(
        recv(env.host.lyrics(ID, track(1008))).unwrap(),
        None,
        "nobody added lyrics"
    );

    // A track from another source, found by title, artist and length.
    let mut foreign = item(ItemKind::Track, "tr:local-77", "Mock Song One");
    foreign.artist = Some("Mock Artist".into());
    foreign.duration_ms = Some(200_900);
    let found = recv(env.host.lyrics(ID, foreign.clone()))
        .unwrap()
        .expect("lyrics");
    assert_eq!(found.text, include_str!("golden/1001-translated.lrc"));
    foreign.duration_ms = Some(260_000);
    assert_eq!(
        recv(env.host.lyrics(ID, foreign)).unwrap(),
        None,
        "another recording"
    );
    let mut stranger = item(ItemKind::Track, "x:1", "No Such Song Anywhere");
    stranger.artist = Some("Nobody".into());
    assert_eq!(recv(env.host.lyrics(ID, stranger)).unwrap(), None);
}

#[test]
fn lyrics_without_translations() {
    let mock = quick_mock();
    let (env, info) = with_mock(dir(), PACKAGE, &mock.server, json!({ "translations": false }));
    let lyrics = recv(env.host.lyrics(&info.id, track(1001))).unwrap().unwrap();
    assert_eq!(lyrics.text, include_str!("golden/1001.lrc"));
}

#[test]
fn artwork() {
    let mock = quick_mock();
    let env = env_for(&mock);
    let base = mock.server.url();
    let album = recv(
        env.host
            .artwork(ID, item(ItemKind::Album, "ncm:al:701", "x"), 600),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        album.url,
        format!("{base}/p1.music.126.net/mock/album-701.jpg?param=600y600")
    );
    assert_eq!((album.width, album.height), (Some(600), Some(600)));
    let fetched = fetch(&album.url, &[]).unwrap();
    assert_eq!((fetched.status, fetched.body.as_slice()), (200, PICTURE));
    let song = recv(env.host.artwork(ID, track(1003), 100)).unwrap().unwrap();
    assert_eq!(
        song.url,
        format!("{base}/p3.music.126.net/mock/album-702.jpg?param=100y100")
    );
    let artist = recv(
        env.host
            .artwork(ID, item(ItemKind::Artist, "ncm:ar:502", "x"), 64),
    )
    .unwrap()
    .unwrap();
    assert!(
        artist
            .url
            .contains("/p2.music.126.net/mock/artist-502.jpg?param=64y64")
    );
    let list = recv(
        env.host
            .artwork(ID, item(ItemKind::Playlist, "ncm:pl:3002", "x"), 3000),
    )
    .unwrap()
    .unwrap();
    assert!(
        list.url
            .ends_with("/p4.music.126.net/mock/playlist-3002.jpg?param=1200y1200")
    );
    // Items from other sources, found by search.
    let mut foreign = item(ItemKind::Track, "local:9", "Mock Song Two");
    foreign.artist = Some("Mock Artist".into());
    let found = recv(env.host.artwork(ID, foreign, 300)).unwrap().unwrap();
    assert!(found.url.contains("/album-701.jpg"));
    let mut foreign_album = item(ItemKind::Album, "local:al", "Mock Members Album");
    foreign_album.artist = Some("Mock Artist".into());
    let found = recv(env.host.artwork(ID, foreign_album, 300)).unwrap().unwrap();
    assert!(found.url.contains("/album-702.jpg"));
    assert_eq!(
        recv(
            env.host
                .artwork(ID, item(ItemKind::Artist, "local:ar", "Mock Artist"), 300)
        )
        .unwrap(),
        None,
        "artists from elsewhere are not looked up"
    );
    // Listed items need no further request.
    recv(
        env.host
            .search(ID, "mock song one", kinds(&[ItemKind::Track]), page(5)),
    )
    .unwrap();
    mock.clear();
    recv(env.host.artwork(ID, track(1001), 200)).unwrap().unwrap();
    assert!(mock.decoded().is_empty());
}

#[test]
fn an_expired_session_asks_to_sign_in_again() {
    let mock = quick_mock();
    let env = env_for(&mock);
    sign_in(&env);
    recv(
        env.host
            .browse(ID, Some(item(ItemKind::Folder, "ncm:f:liked", "Liked")), page(10)),
    )
    .unwrap();
    mock.expire_sessions();
    assert_eq!(
        plugin_error(recv(env.host.browse(
            ID,
            Some(item(ItemKind::Folder, "ncm:f:playlists", "x")),
            page(10)
        ))),
        PluginError::AuthRequired
    );
    assert_eq!(
        plugin_error(recv(env.host.resolve_stream(ID, track(1001), Quality::High))),
        PluginError::AuthRequired
    );
}

#[test]
fn signing_out_ends_the_session() {
    let mock = quick_mock();
    let env = env_for(&mock);
    sign_in(&env);
    recv(env.host.logout(ID)).unwrap();
    assert_eq!(mock.calls("/logout"), 1);
    assert!(env.secrets.get(SECRET).unwrap().is_none());
    assert_eq!(env.host.plugin(ID).unwrap().account, None);
    assert_eq!(
        plugin_error(recv(env.host.browse(ID, None, page(10)))),
        PluginError::AuthRequired
    );
    assert!(mock.violations().is_empty(), "{:?}", mock.violations());
}
