//! The Spotify plugin's component in a real plugin host, against a mock of
//! the Spotify accounts service and Web API (`tests/mock`, invented data).
//! Spotify itself was not reachable from where these tests were written:
//! they check the protocol as documented, not the live service.

mod mock;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

use jf_plugin_host::{
    AuthKind, AuthStep, CallError, Capability, ItemKind, PlaybackMode, PluginError, RemoteDeviceKind,
    RemoteRepeat, SecretStore,
};
use jf_plugin_testkit::*;
use mock::Mock;
use serde_json::json;

const ID: &str = "org.jellyfesh.spotify";

fn crate_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// The plugin pointed at the mock, its redirect port moved to a free one so
/// tests can sign in at the same time.
fn install(mock: &Mock) -> (Env, u16) {
    let port = free_port();
    let mut files = PluginFiles::read(crate_dir()).with_test_server_setting("test_server");
    files.manifest = files
        .manifest
        .replace("redirect_port = 29817", &format!("redirect_port = {port}"));
    assert!(files.manifest.contains(&format!("redirect_port = {port}")));
    let env = Env::new();
    env.install_files(crate_dir(), "jf-spotify", &files);
    env.settings(ID, json!({ "test_server": mock.server.url() }));
    (env, port)
}

/// Signed in without the browser: a Client ID and tokens the mock knows.
fn signed_in(mock: &Mock) -> Env {
    let (env, _) = install(mock);
    {
        let mut state = mock.state.lock().unwrap();
        state.access.push("at-0".into());
        state.refresh.push("rt-0".into());
    }
    set_session(&env, "at-0", "rt-0", u64::MAX / 2);
    env.secrets
        .set(&format!("jellyfesh/{ID}/client-id"), mock::CLIENT_ID)
        .unwrap();
    env
}

fn set_session(env: &Env, access: &str, refresh: &str, expires_at_ms: u64) {
    let session = json!({ "access_token": access, "refresh_token": refresh, "expires_at_ms": expires_at_ms });
    env.secrets
        .set(&format!("jellyfesh/{ID}/session"), &session.to_string())
        .unwrap();
}

fn stored(env: &Env, name: &str) -> Option<String> {
    env.secrets.get(&format!("jellyfesh/{ID}/{name}")).unwrap()
}

/// A `GET` to the loopback listener, as the browser sends it after sign-in.
fn browser_get(url: &str) -> String {
    let url = url::Url::parse(url).unwrap();
    let mut stream = TcpStream::connect(("127.0.0.1", url.port().unwrap())).unwrap();
    let target = format!("{}?{}", url.path(), url.query().unwrap_or_default());
    write!(
        stream,
        "GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    answer
}

#[test]
fn signs_in_with_pkce_and_the_persons_own_client_id() {
    let mock = mock::start();
    let (env, port) = install(&mock);
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let flow = login(&env.host, ID, AuthKind::Oauth);
    // No Client ID yet: the form asks for it and shows the exact Redirect URI.
    let AuthStep::AskFields { fields: form } = next_step(&flow) else {
        panic!("expected the Client ID form");
    };
    assert_eq!(form[0].name, "client-id");
    let help = form[0].help.clone().unwrap();
    assert!(help.contains(&format!("exactly {redirect_uri}")), "{help}");
    assert!(
        help.contains("Development Mode") && help.contains("Premium"),
        "{help}"
    );
    flow.submit(fields(&[("client-id", "not-a-client-id")]));
    let AuthStep::AskFields { fields: again } = next_step(&flow) else {
        panic!("expected the form again");
    };
    assert!(
        again[0]
            .help
            .clone()
            .unwrap()
            .starts_with("That is not a Client ID")
    );
    flow.submit(fields(&[("client-id", mock::CLIENT_ID)]));

    // The authorisation page, with every PKCE parameter.
    let AuthStep::OpenUrl { url } = next_step(&flow) else {
        panic!("expected the authorisation page");
    };
    let url = url::Url::parse(&url).unwrap();
    assert_eq!(url.path(), "/authorize");
    let q = url_query(url.as_str());
    assert_eq!(q["client_id"], mock::CLIENT_ID);
    assert_eq!(q["response_type"], "code");
    assert_eq!(q["redirect_uri"], redirect_uri);
    assert_eq!(q["code_challenge_method"], "S256");
    assert_eq!(q["code_challenge"].len(), 43, "base64url of a SHA-256");
    for scope in [
        "user-read-playback-state",
        "user-modify-playback-state",
        "user-library-read",
        "playlist-read-private",
    ] {
        assert!(q["scope"].split(' ').any(|s| s == scope), "{scope}");
    }
    {
        let mut state = mock.state.lock().unwrap();
        state.challenge = Some(q["code_challenge"].clone());
        state.redirect_uri = Some(redirect_uri.clone());
    }
    let answer = browser_get(&format!("{redirect_uri}?code=good-code&state={}", q["state"]));
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    assert_eq!(
        final_step(&flow),
        AuthStep::Done {
            account: "Mock Listener".into()
        }
    );
    // The code exchange: the verifier matches the challenge, and no secret.
    let exchange = mock.server.requests_to("/api/token").pop().unwrap();
    let form: std::collections::HashMap<String, String> = exchange.form_pairs().into_iter().collect();
    assert_eq!(form["grant_type"], "authorization_code");
    assert_eq!(form["client_id"], mock::CLIENT_ID);
    assert_eq!(form["redirect_uri"], redirect_uri);
    assert_eq!(mock::super_challenge(&form["code_verifier"]), q["code_challenge"]);
    assert!(!form.contains_key("client_secret"));
    assert_eq!(stored(&env, "client-id").as_deref(), Some(mock::CLIENT_ID));
    assert!(stored(&env, "session").unwrap().contains("rt-1"));
    assert!(
        env.files_containing("rt-1").is_empty(),
        "tokens live in the keystore only"
    );

    // Signing in again goes straight to the browser with the stored Client ID.
    let flow = login(&env.host, ID, AuthKind::Oauth);
    assert!(matches!(next_step(&flow), AuthStep::OpenUrl { url } if url.contains(mock::CLIENT_ID)));
}

#[test]
fn tokens_are_renewed_and_rotated_and_invalid_grant_asks_to_sign_in() {
    let mock = mock::start();
    let env = signed_in(&mock);
    // An expired access token: renewed with the refresh token first.
    set_session(&env, "at-gone", "rt-0", 0);
    let playlists = recv(
        env.host
            .browse(ID, Some(item(ItemKind::Folder, "playlists", "x")), page(50)),
    )
    .unwrap();
    assert_eq!(playlists.items.len(), 3);
    let refresh = mock.server.requests_to("/api/token").pop().unwrap();
    let form: std::collections::HashMap<String, String> = refresh.form_pairs().into_iter().collect();
    assert_eq!(form["grant_type"], "refresh_token");
    assert_eq!(form["refresh_token"], "rt-0");
    assert_eq!(form["client_id"], mock::CLIENT_ID);
    let session = stored(&env, "session").unwrap();
    assert!(
        session.contains("rt-1") && !session.contains("rt-0"),
        "the rotated token replaced the old one"
    );
    // A 401 with a fresh-looking token also renews once.
    set_session(&env, "at-revoked", "rt-1", u64::MAX / 2);
    assert!(
        recv(
            env.host
                .browse(ID, Some(item(ItemKind::Folder, "albums", "x")), page(50))
        )
        .is_ok()
    );
    assert!(stored(&env, "session").unwrap().contains("rt-2"));
    // An expired or revoked refresh token: sign in again (the Client ID stays).
    set_session(&env, "at-gone", "rt-revoked", 0);
    assert!(matches!(
        recv(
            env.host
                .browse(ID, Some(item(ItemKind::Folder, "albums", "x")), page(50))
        ),
        Err(CallError::Plugin(PluginError::AuthRequired))
    ));
    assert_eq!(stored(&env, "session"), None);
    assert_eq!(stored(&env, "client-id").as_deref(), Some(mock::CLIENT_ID));
}

#[test]
fn remote_control_drives_the_web_api_player() {
    let mock = mock::start();
    let env = signed_in(&mock);
    let host = &env.host;
    assert!(
        host.plugin(ID)
            .unwrap()
            .capabilities
            .contains(&Capability::Remote)
    );
    assert_eq!(
        recv(host.remote_state(ID)).unwrap(),
        None,
        "nothing plays yet (204)"
    );

    // The restricted TV is hidden: Spotify accepts no commands for it.
    let devices = recv(host.remote_devices(ID)).unwrap();
    let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["Pocket Phone", "Kitchen Speaker"]);
    assert_eq!(devices[1].kind, RemoteDeviceKind::Speaker);

    // An album from its third track, on the kitchen speaker.
    let album = item(ItemKind::Album, mock::ALBUM, "Night Ferry");
    recv(host.remote_play(ID, Some("dev-kitchen".into()), Some(album), Vec::new(), 2)).unwrap();
    let play = mock.server.requests_to("/v1/me/player/play").pop().unwrap();
    assert_eq!(play.method, "PUT");
    assert_eq!(play.query("device_id").as_deref(), Some("dev-kitchen"));
    assert_eq!(
        play.json().unwrap(),
        json!({ "context_uri": format!("spotify:album:{}", mock::ALBUM), "offset": { "position": 2 } })
    );
    let state = recv(host.remote_state(ID)).unwrap().expect("playing");
    assert!(state.playing);
    assert_eq!(state.item.as_ref().unwrap().id, mock::track_id(2));
    assert_eq!(state.device.as_ref().unwrap().id, "dev-kitchen");
    let context = state.context.unwrap();
    assert_eq!(
        (context.kind, context.title.as_str()),
        (ItemKind::Album, "Night Ferry")
    );
    recv(host.remote_state(ID)).unwrap();
    assert_eq!(
        mock.server.count(&format!("/v1/albums/{}", mock::ALBUM)),
        1,
        "the context name is cached"
    );

    // Tracks from an index, and the transport.
    let tracks: Vec<_> = (5..8)
        .map(|n| item(ItemKind::Track, &mock::track_id(n), "t"))
        .collect();
    recv(host.remote_play(ID, None, None, tracks, 1)).unwrap();
    let body = mock
        .server
        .requests_to("/v1/me/player/play")
        .pop()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(body["uris"].as_array().unwrap().len(), 3);
    assert_eq!(body["offset"], json!({ "position": 1 }));
    recv(host.remote_pause(ID)).unwrap();
    recv(host.remote_next(ID)).unwrap();
    recv(host.remote_seek(ID, 30_000)).unwrap();
    recv(host.remote_set_volume(ID, 35)).unwrap();
    recv(host.remote_set_shuffle(ID, true)).unwrap();
    recv(host.remote_set_repeat(ID, RemoteRepeat::All)).unwrap();
    let state = recv(host.remote_state(ID)).unwrap().unwrap();
    assert!(!state.playing && state.shuffle);
    assert_eq!(state.repeat, RemoteRepeat::All);
    assert_eq!(state.position_ms, 30_000);
    assert_eq!(state.item.unwrap().id, mock::track_id(7));
    assert_eq!(state.device.unwrap().volume_percent, Some(35));
    recv(host.remote_previous(ID)).unwrap();
    recv(host.remote_resume(ID)).unwrap();
    recv(host.remote_transfer(ID, "dev-phone".into(), true)).unwrap();
    let transfer = mock
        .server
        .requests_to("/v1/me/player")
        .into_iter()
        .rfind(|r| r.method == "PUT")
        .unwrap();
    assert_eq!(
        transfer.json().unwrap(),
        json!({ "device_ids": ["dev-phone"], "play": true })
    );
    let methods: Vec<(String, String)> = mock
        .server
        .requests()
        .iter()
        .filter(|r| r.path().starts_with("/v1/me/player/") && r.method != "GET")
        .map(|r| {
            (
                r.method.clone(),
                r.target.trim_start_matches("/v1/me/player/").to_owned(),
            )
        })
        .collect();
    for expected in [
        ("PUT", "pause"),
        ("POST", "next"),
        ("PUT", "seek?position_ms=30000"),
        ("PUT", "volume?volume_percent=35"),
        ("PUT", "shuffle?state=true"),
        ("PUT", "repeat?state=context"),
        ("POST", "previous"),
    ] {
        assert!(
            methods.iter().any(|(m, t)| m == expected.0 && t == expected.1),
            "{expected:?} in {methods:?}"
        );
    }
}

#[test]
fn free_accounts_and_rate_limits_are_explained() {
    let mock = mock::start();
    let env = signed_in(&mock);
    mock.state.lock().unwrap().free = true;
    match recv(env.host.remote_pause(ID)) {
        Err(CallError::Plugin(PluginError::Service(message))) => {
            assert_eq!(message, "Spotify Premium is needed to control playback.");
        }
        other => panic!("expected the Premium message, got {other:?}"),
    }
    // Reading devices needs no Premium.
    assert!(recv(env.host.remote_devices(ID)).is_ok());

    // HTTP 429: Retry-After is kept, and nothing is sent until it passes.
    mock.state.lock().unwrap().throttle = Some(120);
    assert!(matches!(
        recv(env.host.remote_devices(ID)),
        Err(CallError::Plugin(PluginError::RateLimited))
    ));
    mock.state.lock().unwrap().throttle = None;
    let before = mock.server.requests().len();
    assert!(matches!(
        recv(env.host.remote_state(ID)),
        Err(CallError::Plugin(PluginError::RateLimited))
    ));
    assert_eq!(
        mock.server.requests().len(),
        before,
        "no request during the pause"
    );
}

#[test]
fn searches_and_browses_the_library() {
    let mock = mock::start();
    let env = signed_in(&mock);
    let found = recv(env.host.search(ID, "harbour", kinds(&[]), page(20))).unwrap();
    let search = mock.server.requests_to("/v1/search").pop().unwrap();
    assert_eq!(
        search.query("type").as_deref(),
        Some("track,artist,album,playlist")
    );
    assert_eq!(
        search.query("limit").as_deref(),
        Some("5"),
        "20 shared by four types"
    );
    assert_eq!(search.query("q").as_deref(), Some("harbour"));
    let kinds_found: Vec<ItemKind> = found.items.iter().map(|i| i.kind).collect();
    assert_eq!(&kinds_found[..5], [ItemKind::Track; 5]);
    assert_eq!(&kinds_found[5..7], [ItemKind::Artist, ItemKind::Album]);
    // Four playlist entries asked for, one of them null: skipped.
    assert_eq!(kinds_found[7..].len(), 2);
    assert_eq!(found.next.as_deref(), Some("5"));
    let track = &found.items[0];
    assert_eq!(track.isrc.as_deref(), Some("QZMCK2600000"));
    assert_eq!(
        track.artwork.as_ref().unwrap().url,
        format!("https://i.scdn.co/image/{}-300", mock::ALBUM)
    );
    let links = env.host.plugin(ID).unwrap().links;
    assert_eq!(
        links.url_for(ItemKind::Track, &track.id).as_deref(),
        Some(format!("https://open.spotify.com/track/{}", track.id).as_str())
    );
    recv(
        env.host
            .search(ID, "harbour", kinds(&[ItemKind::Track]), page(50)),
    )
    .unwrap();
    let search = mock.server.requests_to("/v1/search").pop().unwrap();
    assert_eq!(search.query("limit").as_deref(), Some("10"), "Spotify's maximum");

    let root = recv(env.host.browse(ID, None, page(50))).unwrap();
    let titles: Vec<&str> = root.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(titles, ["Your Playlists", "Saved Albums", "Liked Songs"]);
    let liked = recv(env.host.browse(ID, Some(root.items[2].clone()), page(50))).unwrap();
    assert_eq!((liked.items.len(), liked.total), (50, Some(60)));
    let rest = recv(
        env.host
            .browse(ID, Some(root.items[2].clone()), next_page(liked.next, 50)),
    )
    .unwrap();
    assert_eq!((rest.items.len(), rest.next), (10, None));

    // A playlist (old `track` and new `item` entries; local and removed ones skipped).
    let playlist = item(ItemKind::Playlist, mock::PLAYLIST, "Harbour Mix");
    let entries = recv(env.host.browse(ID, Some(playlist), page(50))).unwrap();
    assert_eq!(entries.items.len(), 3);
    // An album's tracks carry the album they came from.
    let mut album = item(ItemKind::Album, mock::ALBUM, "Night Ferry");
    album.year = Some(2021);
    let tracks = recv(env.host.browse(ID, Some(album), page(50))).unwrap();
    assert_eq!(tracks.items.len(), 4);
    assert_eq!(tracks.items[0].album.as_deref(), Some("Night Ferry"));
    assert_eq!(tracks.items[0].album_id.as_deref(), Some(mock::ALBUM));
    // An artist's albums, ten at a time.
    let artist = item(ItemKind::Artist, mock::ARTIST, "Ada Example");
    let albums = recv(env.host.browse(ID, Some(artist), page(50))).unwrap();
    assert_eq!((albums.items.len(), albums.next.as_deref()), (10, Some("10")));
    // Ids that are not Spotify ids never reach a request path.
    assert!(matches!(
        recv(
            env.host
                .browse(ID, Some(item(ItemKind::Album, "../me/tracks", "x")), page(5))
        ),
        Err(CallError::Plugin(PluginError::NotFound))
    ));
}

#[test]
fn the_published_package_keeps_to_spotify_and_its_terms() {
    let files = PluginFiles::read(crate_dir());
    let manifest = jf_plugin_host::manifest::parse(&files.manifest).unwrap();
    assert_eq!(manifest.api_version.to_string(), "1.1");
    assert_eq!(manifest.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest.playback_mode, PlaybackMode::Remote);
    assert_eq!(manifest.oauth_redirect_port, Some(29817));
    let policy = &manifest.content_policy;
    assert!(!policy.unified_search && !policy.mixed_queue && !policy.lyrics_sync);
    assert_eq!(policy.library_storage, jf_plugin_host::LibraryStorage::None);
    assert_eq!(policy.attribution.as_deref(), Some("spotify"));
    assert!(policy.metadata_ttl_secs.is_some());
    assert_eq!(
        manifest
            .links
            .url_for(ItemKind::Playlist, mock::PLAYLIST)
            .as_deref(),
        Some(format!("https://open.spotify.com/playlist/{}", mock::PLAYLIST).as_str())
    );
    let env = Env::new();
    let info = env.install_crate(crate_dir(), "jf-spotify");
    assert_eq!(info.auth_kinds, [AuthKind::Oauth]);
    assert!(
        env.host
            .set_settings(ID, &json!({ "test_server": "http://127.0.0.1:9" }).to_string())
            .is_err(),
        "the real package has no test server setting"
    );
}
