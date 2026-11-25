//! The Jellyfin plugin: the real component in the real plugin host, against
//! a mock Jellyfin server (see `mock/mod.rs`; its data is invented). No live
//! Jellyfin server was available for these tests.

mod mock;

use std::collections::HashMap;
use std::path::Path;

use jf_plugin_host::{
    AuthKind, AuthStep, CallError, Capability, DownloadQuality, FieldKind, Item, ItemKind, ItemPage,
    LyricFormat, PluginError, Quality, SecretStore, StreamInfo, UnplayableReason,
};
use jf_plugin_testkit::*;
use mock::{Mock, PNG, Running, USERS};
use serde_json::json;

const ID: &str = "org.jellyfesh.jellyfin";
const SESSION: &str = "jellyfesh/org.jellyfesh.jellyfin/session";
const QUICK_CONNECT: &str = "jellyfesh/org.jellyfesh.jellyfin/quick-connect";

const MUSIC: &str = "f1000000000000000000000000000001";
const LIVE: &str = "f1000000000000000000000000000002";
const HARBOUR: &str = "a2000000000000000000000000000001";
const TIDE: &str = "a2000000000000000000000000000002";
const LOW_WATER: &str = "b3000000000000000000000000000001";
const SEA_FOG: &str = "b3000000000000000000000000000002";
const DRIFTWOOD_ALBUM: &str = "b3000000000000000000000000000003";
const EBB: &str = "c4000000000000000000000000000001";
const FLOW: &str = "c4000000000000000000000000000002";
const SLACK_TIDE: &str = "c4000000000000000000000000000003";
const FOG_BELL: &str = "c4000000000000000000000000000004";
const LOST_BUOY: &str = "c4000000000000000000000000000005";
const DRIFTWOOD: &str = "c4000000000000000000000000000006";
const NIGHT_DRIVE: &str = "d5000000000000000000000000000001";
const EMPTY: &str = "d5000000000000000000000000000002";

fn plugin_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// A host with the plugin installed and pointed at `server`.
fn env_for(server: &TestServer) -> Env {
    let env = Env::new();
    env.install_crate(plugin_dir(), "jf-jellyfin");
    env.settings(ID, json!({ "server_url": server.url() }));
    env
}

/// Signs in with a user name and password.
fn sign_in_as(env: &Env, user: &str, password: &str) {
    let flow = login(&env.host, ID, AuthKind::PasswordExchange);
    let AuthStep::AskFields { fields: form } = next_step(&flow) else {
        panic!("expected a form");
    };
    assert_eq!(form.len(), 2);
    flow.submit(fields(&[("username", user), ("password", password)]));
    assert_eq!(final_step(&flow), AuthStep::Done { account: user.into() });
}

fn sign_in(env: &Env) {
    sign_in_as(env, USERS[0].0, USERS[0].1);
}

/// The message of a failed login.
fn failure(flow: &jf_plugin_host::AuthFlow) -> String {
    match final_step(flow) {
        AuthStep::Failed { message } => message,
        other => panic!("expected a failure, got {other:?}"),
    }
}

fn ids(page: &ItemPage) -> Vec<String> {
    page.items.iter().map(|i| i.id.clone()).collect()
}

fn tr(id: &str) -> String {
    format!("tr:{id}")
}

fn al(id: &str) -> String {
    format!("al:{id}")
}

fn ar(id: &str) -> String {
    format!("ar:{id}")
}

fn pl(id: &str) -> String {
    format!("pl:{id}")
}

fn track(id: &str, title: &str) -> Item {
    let mut track = item(ItemKind::Track, id, title);
    track.artist = Some("Harbour Lights".into());
    track
}

/// The `MediaBrowser` authorisation of the last request to `path`.
fn auth_of(server: &Running, path: &str) -> HashMap<String, String> {
    let request = server
        .requests_to(path)
        .pop()
        .unwrap_or_else(|| panic!("no request to {path}"));
    mock::auth_parts(request.header("authorization").expect("an authorization header")).unwrap()
}

fn browse(env: &Env, id: &str, cursor: Option<String>, limit: u32) -> ItemPage {
    recv(env.host.browse(
        ID,
        Some(item(ItemKind::Folder, id, "x")),
        next_page(cursor, limit),
    ))
    .unwrap_or_else(|e| panic!("browse {id}: {e}"))
}

#[test]
fn installs_with_its_manifest() {
    let server = Mock::default().start();
    let env = env_for(&server);
    let info = env.host.plugin(ID).unwrap();
    assert_eq!(info.name, "Jellyfin");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert!(info.official, "signed with the test key, which this host trusts");
    assert_eq!(info.capabilities.len(), 11);
    assert_eq!(info.auth_kinds, vec![AuthKind::Qr, AuthKind::PasswordExchange]);
    assert!(info.has_icon);
    // No quality setting: the app's level decides (ADR-0026).
    assert!(info.settings.get("quality").is_none());
    assert!(info.settings_schema["properties"].get("quality").is_none());
    assert_eq!(
        info.content_policy.qualities,
        vec![Quality::Lossless, Quality::High, Quality::Normal, Quality::Low]
    );
    assert_eq!(info.settings["transcode_codec"], "aac");
    assert_eq!(info.settings["device_name"], "Jellyfesh");
    assert_eq!(info.settings_schema["properties"]["server_url"]["format"], "uri");
    let capabilities = recv(env.host.capabilities(ID)).unwrap();
    assert!(capabilities.contains(&Capability::Recommendations));
    assert!(capabilities.contains(&Capability::Discover) && capabilities.contains(&Capability::Details));
    assert!(capabilities.contains(&Capability::Library) && capabilities.contains(&Capability::Download));
}

#[test]
fn password_sign_in_keeps_only_the_token() {
    let server = Mock::default().start();
    let env = env_for(&server);
    // Nothing works before signing in.
    assert_eq!(
        recv(env.host.search(ID, "a", kinds(&[]), page(10))),
        Err(CallError::Plugin(PluginError::AuthRequired))
    );
    sign_in(&env);
    assert_eq!(env.host.plugin(ID).unwrap().account.as_deref(), Some("alice"));

    // The sign-in named the client and device, without a token.
    let first = auth_of(&server, "/Users/AuthenticateByName");
    assert_eq!(first["Client"], "Jellyfesh");
    assert_eq!(first["Device"], "Jellyfesh");
    assert_eq!(first["Version"], env!("CARGO_PKG_VERSION"));
    let device_id = first["DeviceId"].clone();
    assert_eq!(device_id.len(), 32);
    assert!(device_id.bytes().all(|b| b.is_ascii_hexdigit()));
    assert!(!first.contains_key("Token"));

    // Only the token is kept, in the keystore.
    let sessions = server.sessions();
    assert_eq!(sessions.len(), 1);
    let (token, session) = &sessions[0];
    assert_eq!(
        (session.user.as_str(), session.device_id.as_str()),
        ("alice", device_id.as_str())
    );
    assert_eq!(env.secrets.keys(), vec![SESSION.to_string()]);
    assert!(
        env.secrets
            .get(SESSION)
            .unwrap()
            .unwrap()
            .contains(token.as_str())
    );
    let password = USERS[0].1;
    assert!(!env.secrets.any_value_contains(password));
    assert!(env.files_containing(password).is_empty());
    assert!(env.files_containing(token).is_empty(), "the token is not on disk");

    // Later requests carry the token in the header, never in the URL, and
    // leave the user to the token.
    server.clear();
    recv(env.host.search(ID, "flow", kinds(&[]), page(10))).unwrap();
    for request in server.requests() {
        let auth = mock::auth_parts(request.header("authorization").unwrap()).unwrap();
        assert_eq!(auth.get("Token"), Some(token), "{}", request.target);
        assert_eq!(auth["DeviceId"], device_id);
        assert!(!request.target.contains(token.as_str()), "{}", request.target);
        assert!(request.query("userId").is_none() && request.query("api_key").is_none());
    }
    // The password went to the server once, in the sign-in's body.
    let carrying: Vec<String> = server
        .requests()
        .into_iter()
        .filter(|r| String::from_utf8_lossy(&r.body).contains(password) || r.target.contains("horse"))
        .map(|r| r.target)
        .collect();
    assert!(carrying.is_empty(), "{carrying:?}");
}

#[test]
fn the_device_is_named_and_kept() {
    let server = Mock::default().start();
    let env = env_for(&server);
    env.settings(
        ID,
        json!({ "server_url": server.url(), "device_name": "Kitchen speaker \"2\"" }),
    );
    sign_in(&env);
    let auth = auth_of(&server, "/Users/AuthenticateByName");
    assert_eq!(
        auth["Device"], "Kitchen speaker \"2\"",
        "percent-encoded and decoded"
    );
    let device_id = auth["DeviceId"].clone();

    // The same device after a restart, a logout and a new sign-in.
    let host = env.reopen();
    recv(host.search(ID, "flow", kinds(&[]), page(5))).unwrap();
    assert_eq!(auth_of(&server, "/Items")["DeviceId"], device_id);
    recv(host.logout(ID)).unwrap();
    drop(host);
    sign_in(&env);
    assert_eq!(
        auth_of(&server, "/Users/AuthenticateByName")["DeviceId"],
        device_id
    );
}

#[test]
fn wrong_password() {
    let server = Mock::default().start();
    let env = env_for(&server);
    let flow = login(&env.host, ID, AuthKind::PasswordExchange);
    let _form = next_step(&flow);
    flow.submit(fields(&[("username", "alice"), ("password", "wrong password")]));
    assert_eq!(failure(&flow), "Wrong user name or password.");
    assert!(env.secrets.keys().is_empty());
    let flow = login(&env.host, ID, AuthKind::PasswordExchange);
    let _form = next_step(&flow);
    // A blank user name is refused (by the host already: the field is required).
    flow.submit(fields(&[("username", " "), ("password", "x")]));
    assert!(failure(&flow).contains("User Name"));
    assert!(env.secrets.keys().is_empty());
}

#[test]
fn password_form_is_described() {
    let server = Mock::default().start();
    let env = env_for(&server);
    let flow = login(&env.host, ID, AuthKind::PasswordExchange);
    let AuthStep::AskFields { fields: form } = next_step(&flow) else {
        panic!("expected a form");
    };
    assert_eq!(
        (form[0].name.as_str(), form[0].kind),
        ("username", FieldKind::Text)
    );
    assert!(form[0].required);
    assert!(
        form[1].secret && !form[1].required,
        "accounts may have no password"
    );
    let help = form[1].help.as_deref().unwrap();
    assert!(help.contains("access token") && !help.contains("http,"), "{help}");
    flow.cancel();
}

#[test]
fn quick_connect_sign_in() {
    let server = Mock::default().start();
    let env = env_for(&server);
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let flow = login(&env.host, ID, AuthKind::Qr);
    let AuthStep::ShowCode {
        code,
        hint,
        expires_at_ms,
    } = next_step(&flow)
    else {
        panic!("expected a code");
    };
    assert_eq!(code.len(), 7, "{code:?}");
    assert_eq!(code.as_bytes()[3], b' ', "shown as two groups: {code:?}");
    assert!(hint.contains("Quick Connect"), "{hint}");
    let expires = expires_at_ms.unwrap();
    assert!(
        (started + 9 * 60_000..=started + 11 * 60_000).contains(&expires),
        "{expires}"
    );
    // The request's secret waits in the keystore, not in the flow state.
    assert_eq!(env.secrets.keys(), vec![QUICK_CONNECT.to_string()]);
    let initiate = auth_of(&server, "/QuickConnect/Initiate");
    assert_eq!(initiate["Client"], "Jellyfesh");
    assert!(!initiate.contains_key("Token"));

    // Not approved yet: the host keeps polling and showing the code.
    let again = next_step(&flow);
    assert!(
        matches!(&again, AuthStep::ShowCode { code: c, .. } if *c == code),
        "{again:?}"
    );
    server.approve(&code, "alice");
    assert_eq!(
        final_step(&flow),
        AuthStep::Done {
            account: "alice".into()
        }
    );
    assert!(server.count("/QuickConnect/Connect") >= 2);
    assert_eq!(
        env.secrets.keys(),
        vec![SESSION.to_string()],
        "the request's secret is gone"
    );
    let sessions = server.sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].1.device_id, initiate["DeviceId"]);
    assert!(
        env.secrets
            .get(SESSION)
            .unwrap()
            .unwrap()
            .contains(sessions[0].0.as_str())
    );
    let found = recv(env.host.search(ID, "ebb", kinds(&[ItemKind::Track]), page(5))).unwrap();
    assert_eq!(ids(&found), vec![tr(EBB)]);
}

#[test]
fn quick_connect_turned_off() {
    let server = Mock {
        quick_connect: false,
        ..Mock::default()
    }
    .start();
    let env = env_for(&server);
    let flow = login(&env.host, ID, AuthKind::Qr);
    let message = failure(&flow);
    assert!(message.contains("Quick Connect is turned off"), "{message}");
    assert_eq!(server.count("/QuickConnect/Initiate"), 0);
}

#[test]
fn quick_connect_code_expires() {
    let server = Mock::default().start();
    let env = env_for(&server);
    let flow = login(&env.host, ID, AuthKind::Qr);
    assert!(matches!(next_step(&flow), AuthStep::ShowCode { .. }));
    server.expire_codes();
    let message = failure(&flow);
    assert!(message.contains("expired"), "{message}");
    assert!(env.secrets.keys().is_empty());
}

#[test]
fn unreachable_server() {
    let env = Env::new();
    env.install_crate(plugin_dir(), "jf-jellyfin");
    let down = closed_port_url();
    env.settings(ID, json!({ "server_url": down }));
    for kind in [AuthKind::PasswordExchange, AuthKind::Qr] {
        let flow = login(&env.host, ID, kind);
        let message = failure(&flow);
        assert!(
            message.starts_with(&format!("Cannot reach the server at {down}")),
            "{message}"
        );
    }

    // Signed in, then the server goes away.
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);
    drop(server);
    assert!(matches!(
        recv(env.host.search(ID, "a", kinds(&[]), page(10))),
        Err(CallError::Plugin(PluginError::Network(_)))
    ));
    assert!(matches!(
        recv(env.host.resolve_stream(ID, track(&tr(EBB), "Ebb"), Quality::High)),
        Err(CallError::Plugin(PluginError::Network(_)))
    ));
}

#[test]
fn other_servers_are_explained() {
    let emby = Mock {
        product: "Emby Server",
        version: "4.9.1.80",
        ..Mock::default()
    }
    .start();
    let env = env_for(&emby);
    let message = failure(&login(&env.host, ID, AuthKind::PasswordExchange));
    assert!(message.contains("Emby Server, not Jellyfin"), "{message}");

    let old = Mock {
        version: "10.8.13",
        ..Mock::default()
    }
    .start();
    let env = env_for(&old);
    let message = failure(&login(&env.host, ID, AuthKind::Qr));
    assert!(
        message.contains("10.8.13") && message.contains("10.9"),
        "{message}"
    );

    let web = TestServer::start(|_| Response::text(404, "<html>Welcome to nginx</html>"));
    let env = env_for(&web);
    let message = failure(&login(&env.host, ID, AuthKind::PasswordExchange));
    assert!(message.contains("No Jellyfin server answers"), "{message}");

    let env = Env::new();
    env.install_crate(plugin_dir(), "jf-jellyfin");
    let message = failure(&login(&env.host, ID, AuthKind::Qr));
    assert!(message.contains("Enter your server's address"), "{message}");
}

#[test]
fn search_by_kind_and_page() {
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);

    // "o": one artist, three albums, no playlist, four tracks.
    let mut seen = Vec::new();
    let mut cursor = None;
    for _ in 0..20 {
        let found = recv(env.host.search(ID, "o", kinds(&[]), next_page(cursor.clone(), 5))).unwrap();
        assert!(found.items.len() <= 5);
        seen.extend(ids(&found));
        cursor = found.next;
        if cursor.is_none() {
            break;
        }
    }
    assert!(cursor.is_none(), "the search ends");
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), seen.len(), "no item twice: {seen:?}");
    let mut expected = vec![
        ar(HARBOUR),
        al(LOW_WATER),
        al(SEA_FOG),
        al(DRIFTWOOD_ALBUM),
        tr(FLOW),
        tr(FOG_BELL),
        tr(LOST_BUOY),
        tr(DRIFTWOOD),
    ];
    expected.sort();
    assert_eq!(unique, expected);
    assert_eq!(
        server.count("/Artists"),
        1,
        "artists are searched through /Artists"
    );

    let tracks = recv(env.host.search(ID, "o", kinds(&[ItemKind::Track]), page(50))).unwrap();
    assert_eq!(tracks.items.len(), 4);
    assert!(tracks.items.iter().all(|i| i.kind == ItemKind::Track));
    assert!(tracks.next.is_none());
    let playlists = recv(
        env.host
            .search(ID, "night", kinds(&[ItemKind::Playlist]), page(5)),
    )
    .unwrap();
    assert_eq!(ids(&playlists), vec![pl(NIGHT_DRIVE)]);

    let flow = recv(env.host.search(ID, "flow", kinds(&[ItemKind::Track]), page(5))).unwrap();
    let song = &flow.items[0];
    assert_eq!(song.id, tr(FLOW));
    assert_eq!(song.title, "Flow");
    assert_eq!(song.artist.as_deref(), Some("Harbour Lights, The Quiet Tide"));
    assert_eq!(song.album.as_deref(), Some("Low Water"));
    assert_eq!(song.album_id, Some(al(LOW_WATER)));
    assert_eq!(song.artist_id, Some(ar(HARBOUR)));
    assert_eq!(song.duration_ms, Some(240_000));
    assert_eq!(
        (song.track_number, song.disc_number, song.year),
        (Some(2), Some(1), Some(2022))
    );
    assert!(song.playable);
    // Its own cover, at the listing size, without credentials.
    let art = song.artwork.as_ref().unwrap();
    assert!(
        art.url
            .starts_with(&format!("{}/Items/{FLOW}/Images/Primary?", server.url()))
    );
    let q = url_query(&art.url);
    assert_eq!((q["fillWidth"].as_str(), q["tag"].as_str()), ("300", "own-c2"));
    assert!(art.headers.is_empty());
    // A track without its own image shows its album's.
    let ebb = recv(env.host.search(ID, "ebb", kinds(&[ItemKind::Track]), page(5))).unwrap();
    let art = ebb.items[0].artwork.as_ref().unwrap();
    assert!(
        art.url.contains(&format!("/Items/{LOW_WATER}/Images/Primary?")),
        "{}",
        art.url
    );
    assert_eq!(url_query(&art.url)["tag"], "alb-b1");
    assert_no_user_routes(&server);
}

/// The app asks for each kind on its own and continues with the cursor
/// (Show More, ADR-0024): every page holds only that kind, and nothing
/// repeats. Pages of two here, so the small library needs several.
#[test]
fn search_one_kind_at_a_time_with_show_more() {
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);
    let expected = [
        (ItemKind::Artist, vec![ar(HARBOUR)]),
        (
            ItemKind::Album,
            vec![al(LOW_WATER), al(SEA_FOG), al(DRIFTWOOD_ALBUM)],
        ),
        (ItemKind::Playlist, vec![]),
        (
            ItemKind::Track,
            vec![tr(FLOW), tr(FOG_BELL), tr(LOST_BUOY), tr(DRIFTWOOD)],
        ),
    ];
    for (kind, mut want) in expected {
        let before = server.requests().len();
        let mut seen = Vec::new();
        let mut cursor = None;
        for _ in 0..10 {
            let found = recv(
                env.host
                    .search(ID, "o", kinds(&[kind]), next_page(cursor.clone(), 2)),
            )
            .unwrap();
            assert!(found.items.len() <= 2);
            assert!(found.items.iter().all(|i| i.kind == kind), "{kind:?}");
            seen.extend(ids(&found));
            cursor = found.next;
            if cursor.is_none() {
                break;
            }
        }
        assert!(cursor.is_none(), "{kind:?}: the search ends");
        let mut unique = seen.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), seen.len(), "{kind:?}: nothing twice in {seen:?}");
        want.sort();
        assert_eq!(unique, want, "{kind:?}");
        // Only that kind was asked of the server.
        for request in &server.requests()[before..] {
            let asked = request.query("includeItemTypes");
            match kind {
                ItemKind::Artist => assert_eq!(request.path(), "/Artists"),
                ItemKind::Album => assert_eq!(asked.as_deref(), Some("MusicAlbum")),
                ItemKind::Playlist => assert_eq!(asked.as_deref(), Some("Playlist")),
                _ => assert_eq!(asked.as_deref(), Some("Audio")),
            }
        }
    }
}

#[test]
fn browse_the_library() {
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);

    let root = recv(env.host.browse(ID, None, page(50))).unwrap();
    assert_eq!(
        ids(&root),
        vec![
            format!("f:lib:{MUSIC}:artists"),
            format!("f:lib:{MUSIC}:albums"),
            format!("f:lib:{MUSIC}:recent"),
            "f:playlists".to_string(),
            "f:favourites".to_string(),
        ],
        "one music library: its sections at the top"
    );
    assert!(root.items.iter().all(|i| i.kind == ItemKind::Folder));

    // Artists, one at a time, paged by the server.
    let artists = format!("f:lib:{MUSIC}:artists");
    let first = browse(&env, &artists, None, 1);
    assert_eq!(ids(&first), vec![ar(HARBOUR)]);
    assert_eq!((first.next.as_deref(), first.total), (Some("1"), Some(2)));
    let second = browse(&env, &artists, first.next.clone(), 1);
    assert_eq!(ids(&second), vec![ar(TIDE)]);
    assert!(second.next.is_none());
    assert_eq!(
        server.requests_to("/Artists/AlbumArtists")[0]
            .query("parentId")
            .as_deref(),
        Some(MUSIC)
    );

    // Artist → albums, newest first → tracks.
    let albums = recv(env.host.browse(ID, Some(first.items[0].clone()), page(50))).unwrap();
    assert_eq!(ids(&albums), vec![al(SEA_FOG), al(LOW_WATER)]);
    assert_eq!(albums.items[1].year, Some(2022));
    assert_eq!(albums.items[1].child_count, Some(3));
    assert_eq!(albums.items[1].artist.as_deref(), Some("Harbour Lights"));
    let tracks = recv(env.host.browse(ID, Some(albums.items[1].clone()), page(50))).unwrap();
    assert_eq!(ids(&tracks), vec![tr(EBB), tr(FLOW), tr(SLACK_TIDE)]);
    assert_eq!(tracks.items[0].track_number, Some(1));
    assert_eq!(tracks.items[0].duration_ms, Some(245_000));

    // All albums by name, and the most recently added first.
    let all = browse(&env, &format!("f:lib:{MUSIC}:albums"), None, 50);
    assert_eq!(ids(&all), vec![al(DRIFTWOOD_ALBUM), al(LOW_WATER), al(SEA_FOG)]);
    let recent = browse(&env, &format!("f:lib:{MUSIC}:recent"), None, 50);
    assert_eq!(
        ids(&recent),
        vec![al(SEA_FOG), al(DRIFTWOOD_ALBUM), al(LOW_WATER)]
    );

    // Audio playlists only, then a playlist's tracks in pages.
    let playlists = browse(&env, "f:playlists", None, 50);
    assert_eq!(ids(&playlists), vec![pl(EMPTY), pl(NIGHT_DRIVE)]);
    assert_eq!(playlists.items[1].child_count, Some(4));
    let night = playlists.items[1].clone();
    let part = recv(env.host.browse(ID, Some(night.clone()), page(3))).unwrap();
    assert_eq!(ids(&part), vec![tr(FLOW), tr(DRIFTWOOD), tr(EBB)]);
    assert_eq!((part.next.as_deref(), part.total), (Some("3"), Some(4)));
    let rest = recv(env.host.browse(ID, Some(night), next_page(part.next, 3))).unwrap();
    assert_eq!(ids(&rest), vec![tr(SLACK_TIDE)]);
    assert!(rest.next.is_none());
    assert!(
        recv(env.host.browse(ID, Some(playlists.items[0].clone()), page(5)))
            .unwrap()
            .items
            .is_empty()
    );

    // Favourites.
    let favourites = browse(&env, "f:favourites", None, 50);
    assert_eq!(
        ids(&favourites),
        vec![
            "f:favourites:songs",
            "f:favourites:albums",
            "f:favourites:artists"
        ]
    );
    let lists: Vec<Vec<String>> = favourites
        .items
        .iter()
        .map(|folder| ids(&browse(&env, &folder.id, None, 50)))
        .collect();
    assert_eq!(
        lists,
        vec![
            vec![tr(DRIFTWOOD), tr(EBB), tr(FLOW)],
            vec![al(DRIFTWOOD_ALBUM)],
            vec![ar(TIDE)]
        ]
    );

    // Missing things and things without children.
    assert_eq!(
        recv(
            env.host
                .browse(ID, Some(item(ItemKind::Album, "al:nope", "x")), page(5))
        ),
        Err(CallError::Plugin(PluginError::NotFound))
    );
    assert!(matches!(
        recv(
            env.host
                .browse(ID, Some(item(ItemKind::Track, &tr(EBB), "x")), page(5))
        ),
        Err(CallError::Plugin(PluginError::InvalidInput(_)))
    ));
    for unknown in ["local:music", "f:lib:0000:albums", "f:nothing"] {
        assert_eq!(
            recv(
                env.host
                    .browse(ID, Some(item(ItemKind::Folder, unknown, "x")), page(5))
            ),
            Err(CallError::Plugin(PluginError::NotFound)),
            "{unknown}"
        );
    }
    assert_no_user_routes(&server);
}

#[test]
fn browse_several_libraries() {
    let server = Mock {
        two_libraries: true,
        ..Mock::default()
    }
    .start();
    let env = env_for(&server);
    sign_in(&env);
    let root = recv(env.host.browse(ID, None, page(50))).unwrap();
    assert_eq!(
        ids(&root),
        vec![
            format!("f:lib:{MUSIC}"),
            format!("f:lib:{LIVE}"),
            "f:playlists".to_string(),
            "f:favourites".to_string(),
        ]
    );
    assert_eq!(root.items[1].title, "Live recordings");
    let sections = recv(env.host.browse(ID, Some(root.items[1].clone()), page(50))).unwrap();
    assert_eq!(ids(&sections)[1], format!("f:lib:{LIVE}:albums"));
    let albums = recv(env.host.browse(ID, Some(sections.items[1].clone()), page(50))).unwrap();
    assert_eq!(ids(&albums), vec![al("b3000000000000000000000000000004")]);
    assert_eq!(albums.items[0].artist.as_deref(), Some("Ünïcode & Friends"));
}

fn resolve(env: &Env, id: &str, quality: Quality) -> StreamInfo {
    recv(env.host.resolve_stream(ID, track(&tr(id), "x"), quality)).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn fetched(info: &StreamInfo) -> Fetched {
    let headers: Vec<(String, String)> = info
        .headers
        .iter()
        .map(|h| (h.name.clone(), h.value.clone()))
        .collect();
    fetch(&info.url, &headers).unwrap()
}

#[test]
fn streams_by_quality() {
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);
    let token = server.sessions()[0].0.clone();

    // FLAC: the original for lossless.
    let lossless = resolve(&env, EBB, Quality::Lossless);
    assert!(
        lossless
            .url
            .starts_with(&format!("{}/Audio/{EBB}/universal?", server.url()))
    );
    let q = url_query(&lossless.url);
    assert_eq!(q["deviceId"].len(), 32);
    assert!(q["container"].split(',').any(|c| c == "flac"));
    assert!(q["container"].split(',').any(|c| c == "m4a|aac|alac"));
    assert_eq!(
        (q["audioCodec"].as_str(), q["transcodingContainer"].as_str()),
        ("aac", "aac")
    );
    assert_eq!(q["transcodingProtocol"], "http", "progressive, not HLS");
    assert_eq!(
        (q["maxStreamingBitrate"].as_str(), q["audioBitRate"].as_str()),
        ("100000000", "320000")
    );
    assert!(!lossless.url.contains(&token), "no token in the URL");
    let auth = mock::auth_parts(&lossless.headers[0].value).unwrap();
    assert_eq!(lossless.headers[0].name, "authorization");
    assert_eq!(auth["Token"], token);
    assert_eq!(
        (lossless.codec.as_deref(), lossless.bitrate_kbps),
        (Some("flac"), Some(921))
    );
    assert_eq!(lossless.expires_at_ms, None);
    assert!(lossless.cacheable);
    // The engine fetches it with the header; without it the server refuses.
    let got = fetched(&lossless);
    assert_eq!(
        (got.status, got.body.as_slice()),
        (200, b"original flac".as_slice())
    );
    assert_eq!(got.header("content-type"), Some("audio/flac"));
    assert_eq!(fetch(&lossless.url, &[]).unwrap().status, 401);

    // Above the limit: converted at the limit.
    let high = resolve(&env, EBB, Quality::High);
    assert_eq!(url_query(&high.url)["maxStreamingBitrate"], "320000");
    assert_eq!(
        (high.codec.as_deref(), high.bitrate_kbps),
        (Some("aac"), Some(320))
    );
    assert_eq!(fetched(&high).body, b"converted aac 320000");
    // An MP3 at 320 kbit/s: the original for high, converted for normal.
    let mp3 = resolve(&env, FLOW, Quality::High);
    assert_eq!((mp3.codec.as_deref(), mp3.bitrate_kbps), (Some("mp3"), Some(320)));
    assert_eq!(fetched(&mp3).body, b"original mp3");
    let normal = resolve(&env, FLOW, Quality::Normal);
    assert_eq!(
        (normal.codec.as_deref(), normal.bitrate_kbps),
        (Some("aac"), Some(192))
    );
    assert_eq!(fetched(&normal).body, b"converted aac 192000");
    // AAC in M4A at 256 kbit/s for low; Opus in Ogg as it is.
    assert_eq!(
        fetched(&resolve(&env, SLACK_TIDE, Quality::Low)).body,
        b"converted aac 96000"
    );
    let opus = resolve(&env, DRIFTWOOD, Quality::Normal);
    assert_eq!(
        (opus.codec.as_deref(), opus.bitrate_kbps),
        (Some("opus"), Some(128))
    );
    assert_eq!(fetched(&opus).body, b"original ogg");
    // WMA: the engine cannot play it, so even lossless is converted.
    let wma = resolve(&env, FOG_BELL, Quality::Lossless);
    assert_eq!((wma.codec.as_deref(), wma.bitrate_kbps), (Some("aac"), Some(320)));
    assert_eq!(fetched(&wma).body, b"converted aac 320000");

    // The conversion format is a setting; the quality is always the level
    // the app asks for, and a quality setting is refused as unknown.
    env.settings(
        ID,
        json!({ "server_url": server.url(), "transcode_codec": "opus" }),
    );
    let opus_low = resolve(&env, EBB, Quality::Low);
    assert_eq!(
        (opus_low.codec.as_deref(), opus_low.bitrate_kbps),
        (Some("opus"), Some(96))
    );
    let got = fetched(&opus_low);
    assert_eq!(got.body, b"converted opus 96000");
    assert_eq!(got.header("content-type"), Some("audio/opus"));
    assert_eq!(
        fetched(&resolve(&env, EBB, Quality::Lossless)).body,
        b"original flac",
        "the app's level, not an old setting"
    );
    assert!(
        env.host
            .set_settings(
                ID,
                &json!({ "server_url": server.url(), "quality": "low" }).to_string()
            )
            .is_err()
    );

    assert_eq!(
        recv(env.host.resolve_stream(ID, track("tr:ffff", "x"), Quality::High)),
        Err(CallError::Plugin(PluginError::NotFound))
    );
    assert!(matches!(
        recv(
            env.host
                .resolve_stream(ID, track(&al(LOW_WATER), "x"), Quality::High)
        ),
        Err(CallError::Plugin(PluginError::InvalidInput(_)))
    ));
}

/// Every level the app may ask for, for files of each kind: what the plugin
/// asks Jellyfin for, what it tells the engine, and what the mock server
/// sends back (its answers follow `UniversalAudioController`; the files are
/// invented). The table is `tests/golden/levels.txt`; run with
/// `UPDATE_GOLDEN=1` to accept a new one, then review it.
#[test]
fn levels_map_to_requests() {
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);
    let songs = [
        (EBB, "Ebb (FLAC, 921 kbit/s)"),
        (FLOW, "Flow (MP3, 320 kbit/s)"),
        (SLACK_TIDE, "Slack Tide (AAC in M4A, 256 kbit/s)"),
        (FOG_BELL, "Fog Bell (WMA, 192 kbit/s)"),
        (DRIFTWOOD, "Driftwood (Opus in Ogg, 128 kbit/s)"),
    ];
    let levels = [
        (Quality::Lossless, "lossless"),
        (Quality::High, "high"),
        (Quality::Normal, "normal"),
        (Quality::Low, "low"),
    ];
    let mut table = String::from(
        "# song | level | maxStreamingBitrate | audioBitRate | hint to the engine | the server sent\n",
    );
    for (id, song) in songs {
        for (quality, level) in levels {
            let info = resolve(&env, id, quality);
            let q = url_query(&info.url);
            assert_eq!(q["transcodingProtocol"], "http", "never HLS");
            assert_eq!(q["audioCodec"], q["transcodingContainer"]);
            let hint = match (&info.codec, info.bitrate_kbps) {
                (Some(codec), Some(kbps)) => format!("{codec} {kbps} kbit/s"),
                (Some(codec), None) => codec.clone(),
                (None, _) => "unknown".into(),
            };
            let sent = String::from_utf8(fetched(&info).body).unwrap();
            table.push_str(&format!(
                "{song} | {level} | {} | {} | {hint} | {sent}\n",
                q["maxStreamingBitrate"], q["audioBitRate"]
            ));
        }
    }
    let path = plugin_dir().join("tests/golden/levels.txt");
    if std::env::var("UPDATE_GOLDEN").is_ok_and(|v| v == "1") {
        std::fs::write(&path, &table).unwrap();
    }
    assert_eq!(
        table,
        std::fs::read_to_string(&path).unwrap(),
        "run with UPDATE_GOLDEN=1 to accept the new table"
    );
}

/// Jellyfin 10.9 moved every `/Users/{userId}/…` route (its 12.x releases
/// mark them obsolete); the plugin uses none of them, only the sign-in
/// routes under `/Users`.
fn assert_no_user_routes(server: &Running) {
    let sign_in = [
        "/Users/AuthenticateByName",
        "/Users/AuthenticateWithQuickConnect",
        "/Users/Me",
    ];
    for request in server.requests() {
        let path = request.path();
        assert!(
            !path.starts_with("/Users/") || sign_in.contains(&path),
            "an obsolete route: {path}"
        );
    }
}

/// The library's likes and playlists (ADR-0024): the server's favourites,
/// in name order (Jellyfin keeps no date for them), and its audio
/// playlists, in the pages the app asks for.
#[test]
fn favourites_and_playlists_for_the_library() {
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);
    let likes = |kind, cursor: Option<String>, limit| {
        recv(env.host.library_likes(ID, kind, next_page(cursor, limit))).unwrap()
    };

    let first = likes(ItemKind::Track, None, 2);
    assert_eq!(ids(&first), vec![tr(DRIFTWOOD), tr(EBB)]);
    assert_eq!((first.next.as_deref(), first.total), (Some("2"), Some(3)));
    let rest = likes(ItemKind::Track, first.next.clone(), 2);
    assert_eq!(ids(&rest), vec![tr(FLOW)]);
    assert_eq!(rest.next, None);
    assert_eq!(ids(&likes(ItemKind::Album, None, 50)), vec![al(DRIFTWOOD_ALBUM)]);
    assert_eq!(ids(&likes(ItemKind::Artist, None, 50)), vec![ar(TIDE)]);
    let asked = server.requests_to("/Items");
    assert!(
        asked
            .iter()
            .all(|r| r.query("isFavorite").as_deref() == Some("true"))
    );
    assert_eq!(
        recv(
            env.host
                .library_likes(ID, ItemKind::Playlist, next_page(None, 50))
        ),
        Err(CallError::Plugin(PluginError::Unsupported))
    );

    let lists = recv(env.host.library_playlists(ID, next_page(None, 50))).unwrap();
    let flags: Vec<(String, bool, bool)> = lists
        .entries
        .iter()
        .map(|e| (e.item.id.clone(), e.owned, e.songs_listed))
        .collect();
    assert_eq!(
        flags,
        [(pl(EMPTY), true, true), (pl(NIGHT_DRIVE), true, true)],
        "audio playlists only"
    );
    assert_eq!(lists.total, Some(2));
    assert_eq!(lists.entries[1].item.child_count, Some(4));
    let one = recv(env.host.library_playlists(ID, next_page(None, 1))).unwrap();
    assert_eq!((one.entries.len(), one.next.as_deref()), (1, Some("1")));
    // A playlist's songs, from browse as for any playlist.
    let songs = recv(
        env.host
            .browse(ID, Some(lists.entries[1].item.clone()), next_page(None, 50)),
    )
    .unwrap();
    assert_eq!(songs.items.len(), 4);
    assert_no_user_routes(&server);
}

/// Downloads (ADR-0025): the original file from `/Items/{id}/Download` with
/// its size and name, or Opus through the universal endpoint for a lower
/// quality once the server says the account may download the item; never
/// `/Audio/{id}/stream?static=true`, which skips the download policy.
#[test]
fn downloads_follow_the_download_policy() {
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);
    let download = |id: &str, quality| recv(env.host.download(ID, track(&tr(id), "x"), quality)).unwrap();
    let headers = |info: &jf_plugin_host::DownloadInfo| -> Vec<(String, String)> {
        info.headers
            .iter()
            .map(|h| (h.name.clone(), h.value.clone()))
            .collect()
    };

    let original = download(EBB, DownloadQuality::Original);
    assert_eq!(original.url, format!("{}/Items/{EBB}/Download", server.url()));
    assert_eq!(original.headers.len(), 1);
    assert_eq!(original.headers[0].name, "authorization");
    let auth = mock::auth_parts(&original.headers[0].value).unwrap();
    assert!(!auth["Token"].is_empty(), "the token in the header, not the URL");
    assert_eq!(original.container, "flac");
    assert_eq!(original.size, Some(28_300_000));
    assert!(original.original);
    assert_eq!(original.file_name.as_deref(), Some("01 Ebb.flac"));
    let got = fetch(&original.url, &headers(&original)).unwrap();
    assert_eq!((got.status, got.body.as_slice()), (200, b"file flac".as_slice()));

    // Lower qualities: Opus at the limit, when the file is above it.
    let high = download(EBB, DownloadQuality::High);
    let q = url_query(&high.url);
    assert!(
        high.url
            .starts_with(&format!("{}/Audio/{EBB}/universal?", server.url()))
    );
    assert_eq!(
        (q["audioCodec"].as_str(), q["transcodingContainer"].as_str()),
        ("opus", "opus")
    );
    assert_eq!(
        (q["maxStreamingBitrate"].as_str(), q["audioBitRate"].as_str()),
        ("320000", "320000")
    );
    assert_eq!(q["transcodingProtocol"], "http");
    assert_eq!(
        (
            high.container.as_str(),
            high.original,
            high.size,
            high.file_name.clone()
        ),
        ("opus", false, None, None)
    );
    let got = fetch(&high.url, &headers(&high)).unwrap();
    assert_eq!(got.body, b"converted opus 320000");
    assert_eq!(got.header("content-type"), Some("audio/opus"));
    assert_eq!(
        url_query(&download(FLOW, DownloadQuality::Low).url)["audioBitRate"],
        "128000"
    );
    let small = download(DRIFTWOOD, DownloadQuality::Low);
    assert!(small.original, "Opus at 128 kbit/s is within Low");
    assert_eq!(small.container, "ogg");
    let m4a = download(SLACK_TIDE, DownloadQuality::Original);
    assert_eq!((m4a.container.as_str(), m4a.file_name.clone()), ("m4a", None));
    assert_eq!(download(FOG_BELL, DownloadQuality::Original).container, "wma");

    assert!(matches!(
        recv(
            env.host
                .download(ID, track(&tr(LOST_BUOY), "x"), DownloadQuality::Original)
        ),
        Err(CallError::Plugin(PluginError::Service(_)))
    ));
    assert_eq!(
        recv(
            env.host
                .download(ID, track("tr:ffff", "x"), DownloadQuality::Original)
        ),
        Err(CallError::Plugin(PluginError::NotFound))
    );
    assert!(matches!(
        recv(env.host.download(
            ID,
            item(ItemKind::Album, &al(LOW_WATER), "x"),
            DownloadQuality::Original
        )),
        Err(CallError::Plugin(PluginError::InvalidInput(_)))
    ));
    for request in server.requests() {
        assert!(!request.path().ends_with("/stream"), "{}", request.path());
        assert_ne!(request.query("static").as_deref(), Some("true"));
    }
    assert_no_user_routes(&server);
}

/// Without `EnableContentDownloading` the server answers `CanDownload`
/// false, and the plugin refuses before any file is asked for.
#[test]
fn downloads_off_for_the_account() {
    let server = Mock {
        downloads: false,
        ..Mock::default()
    }
    .start();
    let env = env_for(&server);
    sign_in(&env);
    for quality in [DownloadQuality::Original, DownloadQuality::Low] {
        assert_eq!(
            recv(env.host.download(ID, track(&tr(EBB), "x"), quality)),
            Err(CallError::Plugin(PluginError::Unsupported))
        );
    }
    assert!(server.requests_to(&format!("/Items/{EBB}/Download")).is_empty());
    assert!(server.requests_to(&format!("/Audio/{EBB}/universal")).is_empty());
    // Playing is another policy.
    assert!(recv(env.host.resolve_stream(ID, track(&tr(EBB), "x"), Quality::High)).is_ok());
}

#[test]
fn unplayable_items() {
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);
    let tracks = recv(
        env.host
            .browse(ID, Some(item(ItemKind::Album, &al(SEA_FOG), "Sea Fog")), page(10)),
    )
    .unwrap();
    assert_eq!(ids(&tracks), vec![tr(FOG_BELL), tr(LOST_BUOY)]);
    let missing = &tracks.items[1];
    assert!(tracks.items[0].playable);
    assert!(!missing.playable);
    let reason = missing.unplayable.as_ref().unwrap();
    assert_eq!(reason.reason, UnplayableReason::Unavailable);
    assert!(reason.message.as_deref().unwrap().contains("missing"));
    let error = recv(env.host.resolve_stream(ID, missing.clone(), Quality::High)).unwrap_err();
    assert!(error.to_string().contains("missing"), "{error}");

    // An account without the right to play (and without a password).
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in_as(&env, "bob", "");
    let tracks = recv(
        env.host
            .browse(ID, Some(item(ItemKind::Album, &al(LOW_WATER), "x")), page(10)),
    )
    .unwrap();
    assert!(tracks.items.iter().all(|t| !t.playable));
    assert_eq!(
        tracks.items[0].unplayable.as_ref().unwrap().reason,
        UnplayableReason::Other
    );
    let error = recv(env.host.resolve_stream(ID, track(&tr(EBB), "Ebb"), Quality::High)).unwrap_err();
    assert!(error.to_string().contains("not allowed to play"), "{error}");
    assert_eq!(server.count(&format!("/Audio/{EBB}/universal")), 0);
}

#[test]
fn artwork_by_size() {
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);
    let tracks = recv(
        env.host
            .browse(ID, Some(item(ItemKind::Album, &al(LOW_WATER), "x")), page(10)),
    )
    .unwrap();
    server.clear();
    let art = recv(env.host.artwork(ID, tracks.items[0].clone(), 512))
        .unwrap()
        .unwrap();
    assert!(server.requests().is_empty(), "the image came from the listing");
    assert!(
        art.url
            .starts_with(&format!("{}/Items/{LOW_WATER}/Images/Primary?", server.url()))
    );
    let q = url_query(&art.url);
    assert_eq!(
        (q["fillWidth"].as_str(), q["fillHeight"].as_str()),
        ("512", "512")
    );
    assert_eq!(q["tag"], "alb-b1");
    assert!(art.headers.is_empty(), "Jellyfin serves images without a sign-in");
    let got = fetch(&art.url, &[]).unwrap();
    assert_eq!((got.status, got.body.as_slice()), (200, PNG));

    // An item the plugin has not listed: one lookup.
    let art = recv(
        env.host
            .artwork(ID, item(ItemKind::Album, &al(SEA_FOG), "Sea Fog"), 64),
    )
    .unwrap()
    .unwrap();
    assert!(
        art.url.contains("fillWidth=64") && art.url.contains("tag=alb-b2"),
        "{}",
        art.url
    );
    assert_eq!(server.count("/Items"), 1);
    // No image anywhere, or another source's item.
    assert_eq!(
        recv(env.host.artwork(ID, track(&tr(DRIFTWOOD), "Driftwood"), 300)).unwrap(),
        None
    );
    assert_eq!(
        recv(env.host.artwork(ID, track("local:42", "x"), 300)).unwrap(),
        None
    );
}

fn golden(name: &str) -> String {
    std::fs::read_to_string(plugin_dir().join("tests/golden").join(name)).unwrap()
}

#[test]
fn lyrics_synced_and_unsynced() {
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);
    let lyrics = |item: Item| recv(env.host.lyrics(ID, item)).unwrap();

    let synced = lyrics(track(&tr(EBB), "Ebb")).unwrap();
    assert_eq!(synced.format, LyricFormat::Lrc);
    assert_eq!(synced.text, golden("synced.lrc"));
    let unsynced = lyrics(track(&tr(FLOW), "Flow")).unwrap();
    assert_eq!(unsynced.format, LyricFormat::Plain);
    assert_eq!(unsynced.text, golden("unsynced.txt"));
    assert_eq!(
        lyrics(track(&tr(SLACK_TIDE), "Slack Tide")),
        None,
        "404: no lyrics"
    );

    // A track from another source is matched by title, artist and length.
    let mut foreign = track("local:/music/flow.flac", "Flow");
    foreign.duration_ms = Some(241_000);
    assert_eq!(lyrics(foreign).unwrap().text, golden("unsynced.txt"));
    let mut stranger = track("local:/music/flow.flac", "Flow");
    stranger.duration_ms = Some(400_000);
    assert_eq!(lyrics(stranger), None, "too long to be the same recording");
}

#[test]
fn instant_mix() {
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);
    let mix = recv(
        env.host
            .recommendations_for_item(ID, track(&tr(EBB), "Ebb"), page(10)),
    )
    .unwrap();
    assert_eq!(
        ids(&mix),
        vec![tr(FLOW), tr(DRIFTWOOD), tr(SLACK_TIDE)],
        "the seed itself is left out"
    );
    assert!(mix.next.is_none());
    assert_eq!(
        server.requests_to(&format!("/Items/{EBB}/InstantMix"))[0]
            .query("limit")
            .as_deref(),
        Some("10")
    );
    let artist_mix = recv(env.host.recommendations_for_item(
        ID,
        item(ItemKind::Artist, &ar(HARBOUR), "x"),
        page(2),
    ))
    .unwrap();
    assert_eq!(artist_mix.items.len(), 2);
    let mut foreign = track("local:ebb.flac", "Ebb");
    foreign.duration_ms = Some(245_000);
    let mix = recv(env.host.recommendations_for_item(ID, foreign, page(10))).unwrap();
    assert_eq!(mix.items.len(), 3);
    assert_eq!(
        recv(env.host.recommendations_for_user(ID, page(10))),
        Err(CallError::Plugin(PluginError::Unsupported))
    );
}

#[test]
fn logout_ends_the_session_on_the_server() {
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);
    let token = server.sessions()[0].0.clone();
    recv(env.host.logout(ID)).unwrap();
    assert_eq!(server.logouts(), vec![token.clone()]);
    assert_eq!(auth_of(&server, "/Sessions/Logout")["Token"], token);
    assert!(
        server.sessions().is_empty(),
        "the token stopped working on the server"
    );
    assert!(env.secrets.keys().is_empty());
    assert!(env.host.plugin(ID).unwrap().account.is_none());
    assert_eq!(
        recv(env.host.search(ID, "a", kinds(&[]), page(10))),
        Err(CallError::Plugin(PluginError::AuthRequired))
    );
}

#[test]
fn a_revoked_token_asks_to_sign_in_again() {
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);
    server.revoke_all();
    assert_eq!(
        recv(env.host.browse(ID, None, page(10))),
        Err(CallError::Plugin(PluginError::AuthRequired))
    );
}

#[test]
fn a_sign_in_belongs_to_its_server() {
    let first = Mock::default().start();
    let second = Mock::default().start();
    let env = env_for(&first);
    sign_in(&env);
    let token = first.sessions()[0].0.clone();
    env.settings(ID, json!({ "server_url": second.url() }));
    assert_eq!(
        recv(env.host.search(ID, "a", kinds(&[]), page(10))),
        Err(CallError::Plugin(PluginError::AuthRequired))
    );
    for request in second.requests() {
        let header = request.header("authorization").unwrap_or_default();
        assert!(!header.contains(&token), "the token went to another server");
    }
    // Back to the first server, the sign-in works again.
    env.settings(ID, json!({ "server_url": first.url() }));
    recv(env.host.search(ID, "a", kinds(&[]), page(10))).unwrap();
}

#[test]
fn signed_in_after_a_restart() {
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);
    let host = env.reopen();
    assert_eq!(host.plugin(ID).unwrap().account.as_deref(), Some("alice"));
    let found = recv(host.search(ID, "driftwood", kinds(&[ItemKind::Album]), page(5))).unwrap();
    assert_eq!(ids(&found), vec![al(DRIFTWOOD_ALBUM)]);
}

#[test]
fn discover_shows_latest_resume_favourites_and_suggestions() {
    let server = Mock::default().start();
    let env = env_for(&server);
    assert_eq!(
        recv(env.host.discover(ID)),
        Err(CallError::Plugin(PluginError::AuthRequired))
    );
    sign_in(&env);
    let shelves = recv(env.host.discover(ID)).unwrap();
    let titles: Vec<&str> = shelves.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Recently Added",
            "Continue Listening",
            "Favourites",
            "Suggestions"
        ]
    );
    let latest = &shelves[0];
    assert_eq!(
        latest.items[0].id,
        format!("al:{SEA_FOG}"),
        "the newest album first"
    );
    assert_eq!(
        latest.more.as_deref(),
        Some(format!("f:lib:{MUSIC}:recent").as_str())
    );
    let resumed: Vec<String> = shelves[1].items.iter().map(|i| i.id.clone()).collect();
    assert_eq!(resumed, [format!("tr:{FLOW}")]);
    assert_eq!(shelves[2].more.as_deref(), Some("f:favourites"));
    assert!(shelves[2].items.iter().any(|i| i.id == format!("tr:{EBB}")));
    assert!(shelves[3].items.iter().all(|i| i.kind == ItemKind::Album));
    // Every item with a picture on the server carries it as its cover, which
    // needs no sign-in (Driftwood has none).
    for item in shelves.iter().flat_map(|s| &s.items) {
        let no_picture = item.id == format!("al:{DRIFTWOOD_ALBUM}") || item.id == format!("tr:{DRIFTWOOD}");
        match &item.artwork {
            Some(art) => {
                assert!(
                    art.url.starts_with(&format!("{}/Items/", server.url())),
                    "{}",
                    art.url
                );
                assert!(!art.url.contains("api_key") && art.headers.is_empty());
            }
            None => assert!(no_picture, "{} has no cover", item.id),
        }
    }
    let latest_asked = server.requests_to("/Items/Latest");
    assert_eq!(latest_asked[0].query("groupItems").as_deref(), Some("true"));
    assert_eq!(
        latest_asked[0].query("includeItemTypes").as_deref(),
        Some("Audio")
    );
    let suggestions = server.requests_to("/Items/Suggestions");
    assert_eq!(suggestions[0].query("mediaType").as_deref(), Some("Audio"));
    assert_eq!(suggestions[0].query("type").as_deref(), Some("MusicAlbum"));
    assert_no_user_routes(&server);
}

#[test]
fn details_from_the_overview_genres_studios_and_people() {
    use jf_plugin_host::FactKey;
    let server = Mock::default().start();
    let env = env_for(&server);
    sign_in(&env);
    let facts = |details: &jf_plugin_host::ItemDetails| -> Vec<(FactKey, String)> {
        details
            .facts
            .iter()
            .map(|f| (f.key.clone(), f.value.clone()))
            .collect()
    };
    let album = recv(
        env.host
            .details(ID, item(ItemKind::Album, &format!("al:{LOW_WATER}"), "x")),
    )
    .unwrap();
    assert_eq!(
        album.description.as_deref(),
        Some("Invented notes about Low Water.\nRecorded at an invented pier & harbour.")
    );
    assert_eq!(
        facts(&album),
        [
            (FactKey::Genre, "Folk, Ambient".to_string()),
            (FactKey::Released, "2022-04-01".into()),
            (FactKey::Label, "Invented Label".into())
        ]
    );
    let picture = album.image.expect("the album's picture");
    assert!(
        picture
            .url
            .contains(&format!("/Items/{LOW_WATER}/Images/Primary?"))
    );
    assert!(picture.url.contains("fillWidth=1200"));
    let asked = server.requests_to("/Items");
    let last = asked.last().unwrap();
    assert_eq!(
        last.query("fields").as_deref(),
        Some("Overview,Genres,People,Studios")
    );

    let song = recv(
        env.host
            .details(ID, item(ItemKind::Track, &format!("tr:{EBB}"), "x")),
    )
    .unwrap();
    assert_eq!(
        facts(&song),
        [
            (FactKey::Genre, "Folk".to_string()),
            (FactKey::Released, "2022".into()),
            (FactKey::Composer, "Ada Invented".into()),
            (FactKey::Lyricist, "Bo Invented".into())
        ]
    );
    let artist = recv(
        env.host
            .details(ID, item(ItemKind::Artist, &format!("ar:{HARBOUR}"), "x")),
    )
    .unwrap();
    assert_eq!(
        artist.description.as_deref(),
        Some("An invented band from an invented harbour town.")
    );
    let playlist = recv(
        env.host
            .details(ID, item(ItemKind::Playlist, &format!("pl:{NIGHT_DRIVE}"), "x")),
    )
    .unwrap();
    assert_eq!(
        playlist.description.as_deref(),
        Some("Invented songs for a long drive.")
    );
    assert_eq!(
        recv(env.host.details(ID, item(ItemKind::Track, "local:1", "x"))),
        Err(CallError::Plugin(PluginError::NotFound))
    );
    assert_no_user_routes(&server);
}
