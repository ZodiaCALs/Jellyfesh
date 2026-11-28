//! The NetEase Cloud Music plugin's component in a real plugin host, against
//! a mock of the open platform (`tests/mock`, invented data, a key pair made
//! for each test). NetEase itself was not reachable from where these tests
//! were written: they check the protocol as the research found it, not the
//! live service.

mod mock;

use std::path::Path;

use jf_plugin_host::{
    AuthKind, AuthStep, CallError, ItemKind, PluginError, Quality, SecretStore, UnplayableReason,
};
use jf_plugin_testkit::*;
use mock::Mock;
use serde_json::json;

const ID: &str = "org.jellyfesh.netease";

fn crate_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn install(mock: &Mock) -> Env {
    let env = Env::new();
    let files = PluginFiles::read(crate_dir()).with_test_server_setting("test_server");
    env.install_files(crate_dir(), "jf-netease", &files);
    env.settings(ID, json!({ "test_server": mock.server.url() }));
    env
}

fn secret(env: &Env, name: &str) -> Option<String> {
    env.secrets.get(&format!("jellyfesh/{ID}/{name}")).unwrap()
}

/// Signs in through the form and the QR code; returns the last step.
fn sign_in(env: &Env, app_id: &str, key: &str) -> AuthStep {
    let flow = login(&env.host, ID, AuthKind::Qr);
    let AuthStep::AskFields { fields: form } = next_step(&flow) else {
        panic!("expected the credentials form");
    };
    let names: Vec<&str> = form.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["app-id", "private-key"]);
    assert!(!form[0].secret && form[1].secret);
    flow.submit(fields(&[("app-id", app_id), ("private-key", key)]));
    final_step(&flow)
}

/// Signed in with tokens the mock issued.
fn signed_in(mock: &Mock) -> Env {
    let env = install(mock);
    assert!(matches!(
        sign_in(&env, mock::APP_ID, &mock.key_base64),
        AuthStep::Done { .. }
    ));
    mock.server.clear();
    env
}

#[test]
fn signs_in_with_own_credentials_and_a_qr_code() {
    let mock = mock::start(&[801, 802, 803]);
    let env = install(&mock);
    let flow = login(&env.host, ID, AuthKind::Qr);
    let AuthStep::AskFields { fields: form } = next_step(&flow) else {
        panic!("expected the credentials form");
    };
    assert!(form[0].help.clone().unwrap().contains("developer.music.163.com"));
    assert!(form[1].help.clone().unwrap().contains("keystore"));
    // The key as the console shows it: base64 without armour lines.
    flow.submit(fields(&[
        ("app-id", mock::APP_ID),
        ("private-key", &mock.key_base64),
    ]));
    match next_step(&flow) {
        AuthStep::ShowQr {
            data,
            hint,
            expires_at_ms,
        } => {
            assert_eq!(data, "https://music.163.com/login?codekey=mock-qr-1");
            assert!(hint.unwrap().contains("NetEase Cloud Music app"));
            assert!(expires_at_ms.is_some());
        }
        other => panic!("expected a QR code, got {other:?}"),
    }
    // 801 shows the code again, 802 asks to confirm, 803 signs in.
    assert!(matches!(next_step(&flow), AuthStep::ShowQr { .. }));
    assert!(matches!(next_step(&flow), AuthStep::Waiting { message } if message.contains("Confirm")));
    assert_eq!(
        next_step(&flow),
        AuthStep::Done {
            account: "Mock Listener".into()
        }
    );
    let state = mock.state.lock().unwrap();
    assert_eq!(
        state.bad_signatures, 0,
        "every request was signed with the person's key"
    );
    drop(state);
    // Every request carried the protocol fields; the poll names the App ID.
    let poll = mock
        .server
        .requests_to("/openapi/music/basic/oauth2/device/login/qrcode/get")
        .pop()
        .unwrap();
    let biz: serde_json::Value = serde_json::from_str(&poll.query("bizContent").unwrap()).unwrap();
    assert_eq!(biz, json!({ "clientId": mock::APP_ID, "key": "mock-qr-1" }));
    assert_eq!(poll.query("signType").as_deref(), Some("RSA_SHA256"));
    // The key went to the keystore and nowhere else.
    assert_eq!(
        secret(&env, "private-key").as_deref(),
        Some(mock.key_base64.as_str())
    );
    assert_eq!(secret(&env, "app-id").as_deref(), Some(mock::APP_ID));
    assert!(secret(&env, "session").unwrap().contains("at-1"));
    assert!(
        env.files_containing(&mock.key_base64[40..80]).is_empty(),
        "no file holds the key"
    );
    assert!(env.files_containing("at-1").is_empty(), "no file holds a token");
    // Signing in again skips the form.
    let flow = login(&env.host, ID, AuthKind::Qr);
    assert!(matches!(next_step(&flow), AuthStep::ShowQr { .. }));
}

#[test]
fn a_key_that_cannot_sign_is_asked_for_again() {
    let mock = mock::start(&[803]);
    let env = install(&mock);
    let flow = login(&env.host, ID, AuthKind::Qr);
    let _ = next_step(&flow);
    flow.submit(fields(&[
        ("app-id", mock::APP_ID),
        (
            "private-key",
            "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----",
        ),
    ]));
    let AuthStep::AskFields { fields: form } = next_step(&flow) else {
        panic!("expected the form again");
    };
    let help = form[1].help.clone().unwrap();
    assert!(
        help.starts_with("Jellyfesh cannot use that private key"),
        "{help}"
    );
    assert!(!help.contains("AAAA"), "the message never quotes the key");
    assert_eq!(
        secret(&env, "private-key"),
        None,
        "a key that cannot sign is not kept"
    );
    assert!(mock.server.requests().is_empty(), "nothing was sent");
    // The PKCS #1 PEM form of a good key works.
    flow.submit(fields(&[
        ("app-id", mock::APP_ID),
        ("private-key", &mock.key_pkcs1_pem),
    ]));
    assert!(matches!(final_step(&flow), AuthStep::Done { .. }));
}

#[test]
fn only_the_app_ids_creator_may_sign_in() {
    let mock = mock::start(&[803]);
    let env = install(&mock);
    match sign_in(&env, mock::OTHER_APP_ID, &mock.key_pkcs1_pem) {
        AuthStep::Failed { message } => {
            assert!(message.contains("registered the App ID"), "{message}");
            assert!(
                message.contains("请检查当前授权用户是否与开放平台appId创建者一致"),
                "{message}"
            );
        }
        other => panic!("expected the 301 explanation, got {other:?}"),
    }
    assert_eq!(secret(&env, "session"), None);
}

#[test]
fn an_expired_qr_code_ends_the_sign_in() {
    let mock = mock::start(&[800]);
    let env = install(&mock);
    assert_eq!(
        sign_in(&env, mock::APP_ID, &mock.key_base64),
        AuthStep::Failed {
            message: "The QR code expired. Start again.".into()
        }
    );
}

#[test]
fn browses_playlists_and_marks_what_cannot_play() {
    let mock = mock::start(&[803]);
    let env = signed_in(&mock);
    let root = recv(env.host.browse(ID, None, page(50))).unwrap();
    let titles: Vec<&str> = root.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Created Playlists", "Collected Playlists", "Liked Songs"]
    );
    let created = recv(env.host.browse(ID, Some(root.items[0].clone()), page(2))).unwrap();
    assert_eq!(created.items.len(), 2);
    assert_eq!(created.total, Some(3));
    assert_eq!(
        created.items[0].artwork.as_ref().unwrap().url,
        "https://p1.music.126.net/mock/cover.jpg"
    );
    let rest = recv(
        env.host
            .browse(ID, Some(root.items[0].clone()), next_page(created.next, 2)),
    )
    .unwrap();
    assert_eq!((rest.items.len(), rest.next), (1, None));
    let lists = mock
        .server
        .requests_to("/openapi/music/basic/playlist/created/get/v2");
    let biz: serde_json::Value = serde_json::from_str(&lists[1].query("bizContent").unwrap()).unwrap();
    assert_eq!(biz, json!({ "limit": 2, "offset": 2 }));
    assert!(
        lists
            .iter()
            .all(|r| r.query("accessToken").as_deref() == Some("at-1"))
    );

    let songs = recv(env.host.browse(ID, Some(created.items[0].clone()), page(50))).unwrap();
    let by_id = |id: &str| songs.items.iter().find(|i| i.id == id).unwrap().clone();
    assert!(by_id(mock::FULL).playable);
    assert!(by_id(mock::FREE_LISTEN).playable, "a free full listen plays");
    for (id, reason, words) in [
        (mock::VIP, UnplayableReason::Paywall, "该歌曲为VIP歌曲"),
        (mock::ALBUM_ONLY, UnplayableReason::Paywall, "数字专辑"),
        (mock::PREVIEW, UnplayableReason::Paywall, "does not play previews"),
        (mock::HIDDEN, UnplayableReason::Unavailable, "当前端无版权"),
    ] {
        let item = by_id(id);
        assert!(!item.playable, "{id}");
        let unplayable = item.unplayable.unwrap();
        assert_eq!(unplayable.reason, reason, "{id}");
        assert!(unplayable.message.unwrap().contains(words), "{id}");
    }
    assert_eq!(mock.state.lock().unwrap().bad_signatures, 0);
}

#[test]
fn streams_by_quality_and_never_previews() {
    let mock = mock::start(&[803]);
    let env = signed_in(&mock);
    let track = |id: &str| item(ItemKind::Track, id, "x");
    let stream = recv(env.host.resolve_stream(ID, track(mock::FULL), Quality::High)).unwrap();
    assert!(
        stream.url.ends_with(&format!("/audio/{}-320.mp3", mock::FULL)),
        "{}",
        stream.url
    );
    assert_eq!(
        (stream.codec.as_deref(), stream.bitrate_kbps),
        (Some("mp3"), Some(320))
    );
    assert!(!stream.cacheable);
    let fetched = fetch(&stream.url, &[]).unwrap();
    assert_eq!(fetched.status, 200);
    for (quality, bitrate) in [
        (Quality::Low, 128),
        (Quality::Normal, 128),
        (Quality::Lossless, 999),
    ] {
        let stream = recv(env.host.resolve_stream(ID, track(mock::FULL), quality)).unwrap();
        assert!(
            stream.url.ends_with(&format!("-{bitrate}.mp3")),
            "{quality:?}: {}",
            stream.url
        );
    }
    // The level is always the app's: there is no quality setting any more.
    assert!(
        env.host
            .set_settings(
                ID,
                &json!({ "test_server": mock.server.url(), "quality": "low" }).to_string()
            )
            .is_err()
    );
    let stream = recv(env.host.resolve_stream(ID, track(mock::FULL), Quality::Lossless)).unwrap();
    assert!(stream.url.ends_with("-999.mp3"));
    // A free full listen streams; a preview, a VIP song, no URL and the daily
    // limit do not.
    assert!(
        recv(
            env.host
                .resolve_stream(ID, track(mock::FREE_LISTEN), Quality::High)
        )
        .is_ok()
    );
    for (id, words) in [
        (mock::PREVIEW, "does not play previews"),
        (mock::VIP, "VIP"),
        (mock::NO_URL, "该歌曲暂无音源或暂无播放权限"),
        (mock::CAPPED, "今日播放次数已达上限"),
    ] {
        match recv(env.host.resolve_stream(ID, track(id), Quality::High)) {
            Err(CallError::Plugin(PluginError::Service(message))) => {
                assert!(message.contains(words), "{id}: {message}")
            }
            other => panic!("{id}: expected a refusal, got {other:?}"),
        }
    }
    let detail = mock
        .server
        .requests_to("/openapi/music/basic/song/detail/get/v2")
        .pop()
        .unwrap();
    let biz: serde_json::Value = serde_json::from_str(&detail.query("bizContent").unwrap()).unwrap();
    // High, as the app asked: no setting overrides it.
    assert_eq!(
        biz,
        json!({ "songId": mock::CAPPED, "withUrl": true, "bitrate": 320 })
    );
}

#[test]
fn an_expired_token_is_renewed_once_then_sign_in_is_needed() {
    let mock = mock::start(&[803]);
    let env = signed_in(&mock);
    let session = |access: &str, refresh: &str| {
        env.secrets
            .set(
                &format!("jellyfesh/{ID}/session"),
                &json!({ "access_token": access, "refresh_token": refresh }).to_string(),
            )
            .unwrap();
    };
    // 1406: renewed with the refresh token (rotated), then the call succeeds.
    session("at-expired", "rt-1");
    assert!(
        recv(
            env.host
                .browse(ID, Some(item(ItemKind::Folder, "created", "x")), page(10))
        )
        .is_ok()
    );
    let refresh = mock
        .server
        .requests_to("/openapi/music/basic/user/oauth2/token/refresh/v2")
        .pop()
        .unwrap();
    let biz: serde_json::Value = serde_json::from_str(&refresh.query("bizContent").unwrap()).unwrap();
    assert_eq!(
        biz,
        json!({ "clientId": mock::APP_ID, "clientSecret": "", "refreshToken": "rt-1" })
    );
    assert!(secret(&env, "session").unwrap().contains("rt-2"));
    // A refused refresh: sign in again; the credentials stay.
    session("at-expired", "rt-gone");
    assert!(matches!(
        recv(
            env.host
                .browse(ID, Some(item(ItemKind::Folder, "created", "x")), page(10))
        ),
        Err(CallError::Plugin(PluginError::AuthRequired))
    ));
    assert_eq!(secret(&env, "session"), None);
    assert!(secret(&env, "private-key").is_some());
}

#[test]
fn the_published_package_searches_and_has_no_links() {
    let files = PluginFiles::read(crate_dir());
    let manifest = jf_plugin_host::manifest::parse(&files.manifest).unwrap();
    assert_eq!(
        manifest.api_version.to_string(),
        "1.3",
        "the signing import is API 1.1; Discover and details 1.2; the library and qualities 1.3"
    );
    assert_eq!(manifest.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert!(
        manifest
            .capabilities
            .contains(&jf_plugin_host::Capability::Search)
    );
    // NetEase's terms could not be read: the default stands.
    assert!(manifest.content_policy.unified_search);
    assert!(manifest.links.is_empty());
    assert_eq!(manifest.auth, [AuthKind::Qr]);
    let env = Env::new();
    env.install_crate(crate_dir(), "jf-netease");
    assert!(
        recv(env.host.capabilities(ID))
            .unwrap()
            .contains(&jf_plugin_host::Capability::Search),
        "the plugin says it can search"
    );
    assert!(
        env.host
            .set_settings(ID, &json!({ "test_server": "http://127.0.0.1:9" }).to_string())
            .is_err()
    );
}

/// Every item has a cover on NetEase's picture servers, over https.
fn assert_covers(items: &[jf_plugin_host::Item], what: &str) {
    assert!(!items.is_empty(), "{what}");
    for entry in items {
        let url = &entry
            .artwork
            .as_ref()
            .unwrap_or_else(|| panic!("{what}: {} has no cover", entry.title))
            .url;
        let host = url
            .strip_prefix("https://")
            .and_then(|r| r.split('/').next())
            .unwrap_or_default();
        assert!(host.ends_with(".music.126.net"), "{what}: {url}");
    }
}

#[test]
fn liked_songs_and_discover_come_from_the_accounts_lists() {
    let mock = mock::start(&[803]);
    let env = signed_in(&mock);
    let liked = recv(env.host.browse(
        ID,
        Some(item(ItemKind::Folder, "starred", "Liked Songs")),
        page(50),
    ))
    .unwrap();
    assert_eq!(liked.items.len(), 6, "the liked songs' playlist's songs");
    assert_covers(&liked.items, "Liked Songs");
    let star = mock
        .server
        .requests_to("/openapi/music/basic/playlist/star/get/v2");
    assert_eq!(star[0].query("bizContent").as_deref(), Some("{}"));
    let asked = mock
        .server
        .requests_to("/openapi/music/basic/playlist/song/list/get/v3");
    let biz: serde_json::Value = serde_json::from_str(&asked[0].query("bizContent").unwrap()).unwrap();
    assert_eq!(biz["playlistId"], "pl-liked");

    let shelves = recv(env.host.discover(ID)).unwrap();
    let titles: Vec<&str> = shelves.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Liked Songs", "Created Playlists", "Collected Playlists"]
    );
    assert_eq!(shelves[0].more.as_deref(), Some("starred"));
    assert_eq!(shelves[1].style, jf_plugin_host::ShelfStyle::Grid);
    assert_eq!(
        (shelves[1].items.len(), shelves[1].more.as_deref()),
        (3, Some("created"))
    );
    for shelf in &shelves {
        assert_covers(&shelf.items, &shelf.title);
    }
    assert_eq!(mock.state.lock().unwrap().bad_signatures, 0);
}

/// The library (ADR-0024): the liked songs for Favourites (no liked albums
/// or artists on the open platform), and the created playlists, then the
/// collected ones as followed, for Playlists, across both lists' pages.
#[test]
fn liked_songs_and_playlists_for_the_library() {
    let mock = mock::start(&[803]);
    let env = signed_in(&mock);
    let info = env.host.plugin(ID).unwrap();
    assert_eq!(
        info.content_policy.qualities,
        vec![Quality::Lossless, Quality::High, Quality::Normal, Quality::Low]
    );
    assert!(info.settings_schema["properties"].get("quality").is_none());

    let first = recv(env.host.library_likes(ID, ItemKind::Track, page(4))).unwrap();
    assert_eq!(first.items.len(), 4);
    assert_eq!(first.next.as_deref(), Some("4"));
    let rest = recv(
        env.host
            .library_likes(ID, ItemKind::Track, next_page(first.next, 4)),
    )
    .unwrap();
    assert_eq!(rest.items.len(), 2);
    let asked = mock
        .server
        .requests_to("/openapi/music/basic/playlist/song/list/get/v3");
    let biz: serde_json::Value = serde_json::from_str(&asked[0].query("bizContent").unwrap()).unwrap();
    assert_eq!(biz["playlistId"], "pl-liked");
    for kind in [ItemKind::Album, ItemKind::Artist] {
        assert_eq!(
            recv(env.host.library_likes(ID, kind, page(50))),
            Err(CallError::Plugin(PluginError::Unsupported))
        );
    }

    let all = recv(env.host.library_playlists(ID, page(50))).unwrap();
    let flags: Vec<(&str, bool, bool)> = all
        .entries
        .iter()
        .map(|e| (e.item.id.as_str(), e.owned, e.songs_listed))
        .collect();
    assert_eq!(
        flags,
        [
            ("pl-created-0", true, true),
            ("pl-created-1", true, true),
            ("pl-created-2", true, true),
            ("pl-subed-0", false, true)
        ]
    );
    assert_eq!(all.next, None);
    // In pages of two, the cursor goes on from the created list into the
    // collected one; nothing repeats and nothing is missed.
    let mut seen = Vec::new();
    let mut cursor = None;
    for _ in 0..6 {
        let found = recv(env.host.library_playlists(ID, next_page(cursor, 2))).unwrap();
        assert!(found.entries.len() <= 2);
        seen.extend(found.entries.into_iter().map(|e| (e.item.id, e.owned)));
        cursor = found.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(cursor, None);
    let expected: Vec<(String, bool)> = flags
        .iter()
        .map(|(id, owned, _)| (id.to_string(), *owned))
        .collect();
    assert_eq!(seen, expected);
    assert_eq!(mock.state.lock().unwrap().bad_signatures, 0);
}

/// The business parameters of the last request to `path`.
fn last_biz(mock: &Mock, path: &str) -> serde_json::Value {
    let request = mock
        .server
        .requests_to(path)
        .pop()
        .unwrap_or_else(|| panic!("no request to {path}"));
    serde_json::from_str(&request.query("bizContent").unwrap()).unwrap()
}

const SEARCH_SONG: &str = "/openapi/music/basic/search/song/get/v3";

/// Search (ADR-0027 §5): each kind on its own path, as the core asks, with
/// `keyword`, `limit` and `offset`; the cursor is the offset.
#[test]
fn searches_songs_albums_artists_and_playlists() {
    let mock = mock::start(&[803]);
    let env = signed_in(&mock);
    let search = |kind: ItemKind, cursor: Option<String>| {
        recv(
            env.host
                .search(ID, " tide ", kinds(&[kind]), next_page(cursor, 20)),
        )
        .unwrap()
    };

    let songs = search(ItemKind::Track, None);
    assert_eq!(
        last_biz(&mock, SEARCH_SONG),
        json!({ "keyword": "tide", "limit": 20, "offset": 0 })
    );
    assert_eq!(songs.items.len(), 20);
    assert_eq!((songs.next.as_deref(), songs.total), (Some("20"), Some(25)));
    let first = &songs.items[0];
    assert_eq!(
        (first.kind, first.id.as_str(), first.title.as_str()),
        (ItemKind::Track, "5eac0000000000000000000000000000", "Tide Song 0")
    );
    assert_eq!(
        (first.artist.as_deref(), first.album.as_deref()),
        (Some("Mei Example"), Some("Tide Tables"))
    );
    let artist_id = format!("ar:{}", mock::SEARCH_ARTIST);
    let album_id = format!("al:{}", mock::SEARCH_ALBUM);
    assert_eq!(first.artist_id.as_deref(), Some(artist_id.as_str()));
    assert_eq!(first.album_id.as_deref(), Some(album_id.as_str()));
    assert_eq!(first.duration_ms, Some(200_000));
    assert!(first.playable);
    // NetEase's rules hold in search too: a VIP song needs a purchase.
    let vip = &songs.items[1];
    assert!(!vip.playable);
    assert_eq!(
        vip.unplayable.as_ref().map(|u| u.reason),
        Some(UnplayableReason::Paywall)
    );
    assert_covers(&songs.items, "songs found");
    let rest = search(ItemKind::Track, songs.next);
    assert_eq!(
        last_biz(&mock, SEARCH_SONG),
        json!({ "keyword": "tide", "limit": 20, "offset": 20 })
    );
    assert_eq!((rest.items.len(), rest.next), (5, None));

    let albums = search(ItemKind::Album, None);
    assert_eq!(
        last_biz(&mock, "/openapi/music/basic/search/album/get/v2"),
        json!({ "keyword": "tide", "limit": 20, "offset": 0 })
    );
    let found: Vec<(ItemKind, &str, &str, Option<&str>)> = albums
        .items
        .iter()
        .map(|a| (a.kind, a.id.as_str(), a.title.as_str(), a.artist.as_deref()))
        .collect();
    assert_eq!(
        found,
        [
            (
                ItemKind::Album,
                album_id.as_str(),
                "Tide Tables",
                Some("Mei Example")
            ),
            (
                ItemKind::Album,
                "al:a1b00000000000000000000000000002",
                "Low Tide Live",
                Some("Mei Example")
            ),
        ]
    );
    assert_eq!(albums.next, None);
    assert_covers(&albums.items, "albums found");

    let artists = search(ItemKind::Artist, None);
    assert_eq!(
        last_biz(&mock, "/openapi/music/basic/search/artists/get/v2"),
        json!({ "keyword": "tide", "limit": 20, "offset": 0 })
    );
    assert_eq!(artists.items.len(), 1);
    assert_eq!(
        (
            artists.items[0].kind,
            artists.items[0].id.as_str(),
            artists.items[0].title.as_str()
        ),
        (ItemKind::Artist, artist_id.as_str(), "Mei Example")
    );
    assert_covers(&artists.items, "artists found");

    let playlists = search(ItemKind::Playlist, None);
    assert_eq!(
        last_biz(&mock, "/openapi/music/basic/search/playlist/get/v2"),
        json!({ "keyword": "tide", "limit": 20, "offset": 0 })
    );
    let list = &playlists.items[0];
    assert_eq!(
        (
            list.kind,
            list.id.as_str(),
            list.artist.as_deref(),
            list.child_count
        ),
        (ItemKind::Playlist, "pl-created-0", Some("Mock Listener"), Some(6))
    );
    assert_covers(&playlists.items, "playlists found");

    // Every kind at once (none named) shares the page: five of each.
    let all = recv(env.host.search(ID, "tide", kinds(&[]), page(20))).unwrap();
    let counted = |kind: ItemKind| all.items.iter().filter(|i| i.kind == kind).count();
    assert_eq!(
        [
            ItemKind::Track,
            ItemKind::Album,
            ItemKind::Artist,
            ItemKind::Playlist
        ]
        .map(counted),
        [5, 2, 1, 1]
    );
    assert_eq!(all.next.as_deref(), Some("5"));
    assert_eq!(last_biz(&mock, SEARCH_SONG)["limit"], 5);
    assert_eq!(mock.state.lock().unwrap().bad_signatures, 0);
}

#[test]
fn a_search_finds_nothing_or_fails_in_plain_words() {
    let mock = mock::start(&[803]);
    let env = signed_in(&mock);
    let songs = |query: &str, cursor: Option<String>| {
        recv(
            env.host
                .search(ID, query, kinds(&[ItemKind::Track]), next_page(cursor, 20)),
        )
    };
    let nothing = songs("nothing", None).unwrap();
    assert!(nothing.items.is_empty());
    assert_eq!((nothing.next, nothing.total), (None, Some(0)));
    assert_eq!(
        songs("   ", None).unwrap().items.len(),
        0,
        "an empty search asks nothing"
    );
    assert_eq!(mock.server.count(SEARCH_SONG), 1);
    // Kinds NetEase cannot search are no results, not an error.
    assert!(
        recv(env.host.search(ID, "tide", kinds(&[ItemKind::Folder]), page(20)))
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        songs("garbled", None),
        Err(CallError::Plugin(PluginError::Service(
            "NetEase Cloud Music sent the songs it found in a form Jellyfesh cannot read.".into()
        )))
    );
    assert_eq!(
        songs("slow down", None),
        Err(CallError::Plugin(PluginError::RateLimited))
    );
    assert!(matches!(
        songs("tide", Some("page two".into())),
        Err(CallError::Plugin(PluginError::InvalidInput(_)))
    ));
}

/// An album found by search opens to its songs, an artist to their popular
/// songs; the ids go to NetEase without Jellyfesh's prefixes.
#[test]
fn albums_and_artists_open_to_their_songs() {
    let mock = mock::start(&[803]);
    let env = signed_in(&mock);
    let album = item(
        ItemKind::Album,
        &format!("al:{}", mock::SEARCH_ALBUM),
        "Tide Tables",
    );
    let first = recv(env.host.browse(ID, Some(album.clone()), page(4))).unwrap();
    assert_eq!(
        last_biz(&mock, "/openapi/music/basic/album/song/list/get/v2"),
        json!({ "albumId": mock::SEARCH_ALBUM, "limit": 4, "offset": 0 })
    );
    assert_eq!((first.items.len(), first.next.as_deref()), (4, Some("4")));
    let rest = recv(env.host.browse(ID, Some(album), next_page(first.next, 4))).unwrap();
    assert_eq!((rest.items.len(), rest.next), (2, None));

    let artist = item(
        ItemKind::Artist,
        &format!("ar:{}", mock::SEARCH_ARTIST),
        "Mei Example",
    );
    let popular = recv(env.host.browse(ID, Some(artist), page(50))).unwrap();
    assert_eq!(
        last_biz(&mock, "/openapi/music/basic/artists/hotsong/list/get/v2"),
        json!({ "artistId": mock::SEARCH_ARTIST })
    );
    assert_eq!((popular.items.len(), popular.next), (6, None));
    assert!(popular.items.iter().all(|s| s.kind == ItemKind::Track));
    assert_eq!(mock.state.lock().unwrap().bad_signatures, 0);
}

/// Liked songs NetEase's answers do not let Jellyfesh read are an error in
/// plain words on Favourites and in the folder, never an empty list; the
/// Discover page goes on without them.
#[test]
fn unreadable_liked_songs_are_an_error_not_an_empty_list() {
    let mock = mock::start(&[803]);
    let env = signed_in(&mock);
    let likes = || recv(env.host.library_likes(ID, ItemKind::Track, page(50)));
    let folder = || {
        recv(env.host.browse(
            ID,
            Some(item(ItemKind::Folder, "starred", "Liked Songs")),
            page(50),
        ))
    };
    let unnamed = Err(CallError::Plugin(PluginError::Service(
        "NetEase Cloud Music didn't say which playlist holds your liked songs.".into(),
    )));
    for star in [
        json!(null),
        json!({}),
        json!({ "name": "Liked", "trackCount": 6 }),
    ] {
        mock.state.lock().unwrap().star = Some(star.clone());
        assert_eq!(likes(), unnamed, "{star}");
        assert_eq!(folder(), unnamed, "{star}");
    }
    let shelves = recv(env.host.discover(ID)).unwrap();
    let titles: Vec<&str> = shelves.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["Created Playlists", "Collected Playlists"]);

    // The playlist named under another key still reads.
    mock.state.lock().unwrap().star = Some(json!({ "playlist": { "id": "pl-liked", "trackCount": 6 } }));
    assert_eq!(likes().unwrap().items.len(), 6);

    // Songs NetEase says are there but does not send in a readable form.
    let unread = Err(CallError::Plugin(PluginError::Service(
        "NetEase Cloud Music sent your liked songs in a form Jellyfesh cannot read.".into(),
    )));
    mock.state.lock().unwrap().star = None;
    for songs in [
        json!({}),
        json!({ "records": [] }),
        json!({ "records": [{ "privilege": {} }] }),
        json!({ "result": { "songs": [] } }),
    ] {
        mock.state.lock().unwrap().liked_songs = Some(songs.clone());
        assert_eq!(likes(), unread, "{songs}");
        assert_eq!(folder(), unread, "{songs}");
    }
    // No liked songs at all is an empty list.
    mock.state.lock().unwrap().star = Some(json!({ "id": "pl-liked", "trackCount": 0 }));
    mock.state.lock().unwrap().liked_songs = Some(json!({ "records": [], "recordCount": 0 }));
    let none = likes().unwrap();
    assert_eq!((none.items.len(), none.next), (0, None));
}

#[test]
fn details_of_a_playlist() {
    use jf_plugin_host::FactKey;
    let mock = mock::start(&[803]);
    let env = signed_in(&mock);
    let details = recv(
        env.host
            .details(ID, item(ItemKind::Playlist, "pl-created-0", "created list 0")),
    )
    .unwrap();
    assert_eq!(
        details.description.as_deref(),
        Some("Invented songs for a slow tide.")
    );
    assert_eq!(
        details.image.unwrap().url,
        "https://p1.music.126.net/mock/cover.jpg"
    );
    let facts: Vec<(FactKey, String)> = details
        .facts
        .iter()
        .map(|f| (f.key.clone(), f.value.clone()))
        .collect();
    assert_eq!(
        facts,
        [
            (FactKey::Popularity, "1,234 plays".to_string()),
            (FactKey::Other("Tags".into()), "Chill, 华语".into()),
            (FactKey::Other("Created".into()), "2023-11-14".into()),
        ]
    );
    assert_eq!(
        recv(env.host.details(ID, item(ItemKind::Playlist, "pl-gone", "x"))),
        Err(CallError::Plugin(PluginError::NotFound))
    );
    assert_eq!(
        recv(
            env.host
                .details(ID, item(ItemKind::Track, mock::FULL, "Tidal Clock"))
        ),
        Err(CallError::Plugin(PluginError::Unsupported))
    );
}
