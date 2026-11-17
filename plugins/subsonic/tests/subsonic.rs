//! The Subsonic plugin: the real component in the real plugin host, against
//! a mock OpenSubsonic server (see `mock/mod.rs`; its data is invented).
//! No live Navidrome, Gonic or Airsonic was available for these tests.

mod mock;

use std::path::Path;

use jf_plugin_host::{
    AuthKind, AuthStep, CallError, Capability, FieldKind, Item, ItemKind, LyricFormat, PluginError, Quality,
    SecretStore, UnplayableReason,
};
use jf_plugin_testkit::*;
use mock::{API_KEY, AUDIO, Mock, PNG, USERS};
use serde_json::json;

const ID: &str = "org.jellyfesh.subsonic";
const SECRET: &str = "jellyfesh/org.jellyfesh.subsonic/login";

fn plugin_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// A host with the plugin installed and pointed at `server`.
fn env_for(server: &TestServer) -> Env {
    let env = Env::new();
    env.install_crate(plugin_dir(), "jf-subsonic");
    env.settings(ID, json!({ "server_url": server.url() }));
    env
}

fn password() -> &'static str {
    USERS[0].1
}

/// Signs in with the API key (the server must offer API keys).
fn sign_in_with_key(env: &Env) {
    let flow = login(&env.host, ID, AuthKind::ApiKey);
    let AuthStep::AskFields { fields: form } = next_step(&flow) else {
        panic!("expected a form");
    };
    assert_eq!(form.len(), 1);
    assert!(form[0].secret && form[0].name == "api-key");
    flow.submit(fields(&[("api-key", API_KEY)]));
    assert_eq!(
        final_step(&flow),
        AuthStep::Done {
            account: "alice".into()
        }
    );
}

/// Signs in with alice's password (the server must take tokens).
fn sign_in_with_password(env: &Env) {
    let flow = login(&env.host, ID, AuthKind::PasswordExchange);
    let AuthStep::AskFields { fields: form } = next_step(&flow) else {
        panic!("expected a form");
    };
    assert_eq!(form[0].name, "username");
    assert!(form[1].secret && form[1].name == "password");
    flow.submit(fields(&[("username", "alice"), ("password", password())]));
    assert_eq!(
        final_step(&flow),
        AuthStep::Done {
            account: "alice".into()
        }
    );
}

/// The message of a failed login.
fn failure(flow: &jf_plugin_host::AuthFlow) -> String {
    match final_step(flow) {
        AuthStep::Failed { message } => message,
        other => panic!("expected a failure, got {other:?}"),
    }
}

fn ids(page: &jf_plugin_host::ItemPage) -> Vec<String> {
    page.items.iter().map(|i| i.id.clone()).collect()
}

fn track(id: &str, title: &str) -> Item {
    let mut track = item(ItemKind::Track, id, title);
    track.artist = Some("Northern Lights Orchestra".into());
    track
}

#[test]
fn installs_with_its_manifest() {
    let server = Mock::navidrome_like().start();
    let env = env_for(&server);
    let info = env.host.plugin(ID).unwrap();
    assert_eq!(info.name, "Subsonic");
    assert_eq!(info.version, "0.0.6");
    assert!(info.official, "signed with the test key, which this host trusts");
    assert_eq!(info.capabilities.len(), 7);
    assert_eq!(
        info.auth_kinds,
        vec![AuthKind::ApiKey, AuthKind::PasswordExchange]
    );
    assert!(info.has_icon);
    assert_eq!(info.settings["quality"], "app");
    assert_eq!(info.settings["transcode_format"], "mp3");
    assert_eq!(info.settings_schema["properties"]["server_url"]["format"], "uri");
    let capabilities = recv(env.host.capabilities(ID)).unwrap();
    assert!(capabilities.contains(&Capability::Recommendations));
}

#[test]
fn api_key_sign_in() {
    let server = Mock::with_api_keys().start();
    let env = env_for(&server);
    // Nothing works before signing in.
    assert_eq!(
        recv(env.host.search(ID, "a", kinds(&[]), page(10))),
        Err(CallError::Plugin(PluginError::AuthRequired))
    );
    sign_in_with_key(&env);
    assert_eq!(env.host.plugin(ID).unwrap().account.as_deref(), Some("alice"));
    let stored = env
        .secrets
        .get(SECRET)
        .unwrap()
        .expect("the sign-in is in the keystore");
    assert!(stored.contains(API_KEY));
    assert!(env.files_containing(API_KEY).is_empty(), "the key is not on disk");

    server.clear();
    recv(env.host.search(ID, "polar", kinds(&[]), page(10))).unwrap();
    for (path, params) in mock::all_params(&server) {
        assert_eq!(params.get("apiKey").map(String::as_str), Some(API_KEY), "{path}");
        for key in ["u", "t", "s", "p"] {
            assert!(!params.contains_key(key), "{path} carries {key}");
        }
    }
}

#[test]
fn api_key_rejected() {
    let server = Mock::with_api_keys().start();
    let env = env_for(&server);
    let flow = login(&env.host, ID, AuthKind::ApiKey);
    let _form = next_step(&flow);
    flow.submit(fields(&[("api-key", "not-the-key")]));
    assert!(failure(&flow).contains("did not accept this API key"));
    assert!(env.secrets.keys().is_empty());
}

#[test]
fn password_sign_in_keeps_only_a_token() {
    let server = Mock::navidrome_like().start();
    let env = env_for(&server);
    sign_in_with_password(&env);
    recv(env.host.search(ID, "o", kinds(&[]), page(10))).unwrap();
    recv(env.host.browse(ID, None, page(10))).unwrap();

    // The password never travelled: not even to the server.
    for request in server.requests() {
        let raw = format!("{} {}", request.target, String::from_utf8_lossy(&request.body));
        assert!(!raw.contains("horse"), "{raw}");
    }
    assert!(!env.secrets.any_value_contains(password()));
    assert!(env.files_containing(password()).is_empty());
    // One salt and token pair, made once.
    let signed: Vec<_> = mock::all_params(&server)
        .into_iter()
        .filter(|(_, p)| p.contains_key("t"))
        .collect();
    assert!(signed.len() >= 3);
    let salt = signed[0].1["s"].clone();
    assert!(salt.len() >= 6);
    for (_, params) in &signed {
        assert_eq!(params["u"], "alice");
        assert_eq!(params["s"], salt);
        assert_eq!(params["t"], mock::md5_hex(&format!("{}{salt}", password())));
    }
    let stored = env.secrets.get(SECRET).unwrap().unwrap();
    assert!(stored.contains(&salt));
}

#[test]
fn form_post_keeps_credentials_out_of_urls() {
    let server = Mock::navidrome_like().start();
    let env = env_for(&server);
    sign_in_with_password(&env);
    server.clear();
    recv(env.host.browse(
        ID,
        Some(item(ItemKind::Album, "al:al-1", "Polar Night")),
        page(10),
    ))
    .unwrap();
    let requests = server.requests();
    assert!(!requests.is_empty());
    for request in requests
        .iter()
        .filter(|r| r.path() != "/rest/getOpenSubsonicExtensions.view")
    {
        assert_eq!(request.method, "POST", "{}", request.target);
        assert!(!request.target.contains("t="), "{}", request.target);
        assert!(String::from_utf8_lossy(&request.body).contains("t="));
    }

    // Without formPost, arguments go in the query string.
    let server = Mock::with_api_keys().start();
    let env = env_for(&server);
    sign_in_with_key(&env);
    server.clear();
    recv(env.host.browse(
        ID,
        Some(item(ItemKind::Album, "al:al-1", "Polar Night")),
        page(10),
    ))
    .unwrap();
    assert!(server.requests().iter().all(|r| r.method == "GET"));
}

#[test]
fn password_sign_in_prefers_api_keys() {
    let server = Mock::with_api_keys().start();
    let env = env_for(&server);
    let flow = login(&env.host, ID, AuthKind::PasswordExchange);
    let message = failure(&flow);
    assert!(message.contains("API key"), "{message}");
    assert!(message.contains("mocksonic"), "{message}");
}

#[test]
fn api_key_needs_server_support() {
    let server = Mock::navidrome_like().start();
    let env = env_for(&server);
    let flow = login(&env.host, ID, AuthKind::ApiKey);
    assert!(failure(&flow).contains("does not offer API keys"));
}

#[test]
fn wrong_password() {
    let server = Mock::navidrome_like().start();
    let env = env_for(&server);
    let flow = login(&env.host, ID, AuthKind::PasswordExchange);
    let _form = next_step(&flow);
    flow.submit(fields(&[("username", "alice"), ("password", "wrong password")]));
    assert_eq!(failure(&flow), "Wrong user name or password.");
    assert!(env.secrets.keys().is_empty());
}

#[test]
fn token_sign_in_refused_by_the_server() {
    let server = Mock {
        api_keys: false,
        token_auth: false,
        ..Mock::navidrome_like()
    }
    .start();
    let env = env_for(&server);
    let flow = login(&env.host, ID, AuthKind::PasswordExchange);
    let _form = next_step(&flow);
    flow.submit(fields(&[("username", "alice"), ("password", password())]));
    let message = failure(&flow);
    assert!(message.contains("does not accept token sign-in"), "{message}");
    assert!(
        message.contains("https://music.example/settings/api-keys"),
        "{message}"
    );
}

#[test]
fn password_inside_the_user_name_is_not_kept() {
    let server = Mock::navidrome_like().start();
    let env = env_for(&server);
    let flow = login(&env.host, ID, AuthKind::PasswordExchange);
    let _form = next_step(&flow);
    let (user, password) = USERS[1];
    flow.submit(fields(&[("username", user), ("password", password)]));
    let message = failure(&flow);
    assert!(
        message.contains("password is part of your user name"),
        "{message}"
    );
    assert!(env.secrets.keys().is_empty());
}

#[test]
fn old_servers_are_explained() {
    let server = Mock::old().start();
    let env = env_for(&server);
    let flow = login(&env.host, ID, AuthKind::PasswordExchange);
    let message = failure(&flow);
    assert!(
        message.contains("1.12.0") && message.contains("1.13.0"),
        "{message}"
    );
}

#[test]
fn plain_subsonic_server() {
    // No OpenSubsonic, and unknown methods answer an HTTP 404 page: the
    // plugin learns the API version from ping and signs in with a token.
    let server = Mock {
        open_subsonic: false,
        version: "1.16.1",
        form_post: false,
        song_lyrics: false,
        unknown_404: true,
        ..Mock::navidrome_like()
    }
    .start();
    let env = env_for(&server);
    sign_in_with_password(&env);
    assert!(
        server.count("/rest/ping.view") >= 2,
        "discovery and the sign-in check"
    );
    let found = recv(env.host.search(ID, "aurora", kinds(&[ItemKind::Track]), page(5))).unwrap();
    assert_eq!(ids(&found), vec!["tr:tr-1"]);
    assert!(
        server.requests().iter().all(|r| r.method == "GET"),
        "no form POST"
    );
    let plain = recv(env.host.lyrics(ID, track("tr:tr-3", "Snowfall")))
        .unwrap()
        .unwrap();
    assert_eq!(plain.text, golden("plain-fallback.txt"));
}

#[test]
fn unreachable_server() {
    let env = Env::new();
    env.install_crate(plugin_dir(), "jf-subsonic");
    let down = closed_port_url();
    env.settings(ID, json!({ "server_url": down }));
    let flow = login(&env.host, ID, AuthKind::ApiKey);
    let message = failure(&flow);
    assert!(
        message.starts_with(&format!("Cannot reach the server at {down}")),
        "{message}"
    );

    // Signed in, then the server goes away.
    let server = Mock::with_api_keys().start();
    let env = env_for(&server);
    sign_in_with_key(&env);
    drop(server);
    assert!(matches!(
        recv(env.host.search(ID, "a", kinds(&[]), page(10))),
        Err(CallError::Plugin(PluginError::Network(_)))
    ));
    assert!(matches!(
        recv(
            env.host
                .resolve_stream(ID, track("tr:tr-1", "Aurora"), Quality::High)
        ),
        Err(CallError::Plugin(PluginError::Network(_)))
    ));
}

#[test]
fn not_a_subsonic_server() {
    let server = TestServer::start(|_| Response::text(404, "<html>Welcome to nginx</html>"));
    let env = env_for(&server);
    let flow = login(&env.host, ID, AuthKind::PasswordExchange);
    let message = failure(&flow);
    assert!(
        message.contains("No Subsonic server answers at this address"),
        "{message}"
    );
}

#[test]
fn no_server_address_yet() {
    let env = Env::new();
    env.install_crate(plugin_dir(), "jf-subsonic");
    let flow = login(&env.host, ID, AuthKind::PasswordExchange);
    assert!(failure(&flow).contains("Enter your server's address"));
    assert!(matches!(
        recv(env.host.search(ID, "a", kinds(&[]), page(10))),
        Err(CallError::Plugin(PluginError::InvalidInput(_)))
    ));
}

#[test]
fn search_by_kind_and_page() {
    let server = Mock::with_api_keys().start();
    let env = env_for(&server);
    sign_in_with_key(&env);

    // "o" matches two artists, two albums and three songs.
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
    for expected in [
        "ar:ar-1", "ar:ar-3", "al:al-1", "al:al-2", "tr:tr-1", "tr:tr-3", "tr:tr-5",
    ] {
        assert!(
            seen.iter().any(|id| id == expected),
            "{expected} missing from {seen:?}"
        );
    }
    assert_eq!(seen.len(), 7, "{seen:?}");

    let tracks = recv(env.host.search(ID, "o", kinds(&[ItemKind::Track]), page(50))).unwrap();
    assert!(tracks.items.iter().all(|i| i.kind == ItemKind::Track));
    assert!(tracks.next.is_none());
    let snow = recv(
        env.host
            .search(ID, "snowfall", kinds(&[ItemKind::Track]), page(5)),
    )
    .unwrap();
    let song = &snow.items[0];
    assert_eq!(song.id, "tr:tr-3");
    assert_eq!(song.album_id.as_deref(), Some("al:al-1"));
    assert_eq!(song.artist_id.as_deref(), Some("ar:ar-1"));
    assert_eq!(song.duration_ms, Some(243_000));
    assert!(song.explicit);
    assert!(
        song.artwork.is_none(),
        "cover URLs carry the sign-in and come from `artwork`"
    );
    let none = recv(env.host.search(ID, "o", kinds(&[ItemKind::Playlist]), page(5))).unwrap();
    assert!(none.items.is_empty() && none.next.is_none());
}

#[test]
fn browse_the_library() {
    let server = Mock::with_api_keys().start();
    let env = env_for(&server);
    sign_in_with_key(&env);
    let browse = |node: Option<Item>, cursor: Option<String>, limit: u32| {
        recv(env.host.browse(ID, node, next_page(cursor, limit))).unwrap()
    };

    let root = browse(None, None, 50);
    assert_eq!(
        ids(&root),
        vec!["f:artists", "f:albums", "f:playlists", "f:starred"]
    );
    assert!(root.items.iter().all(|i| i.kind == ItemKind::Folder));

    // Artists, two at a time.
    let first = browse(Some(root.items[0].clone()), None, 2);
    assert_eq!(ids(&first), vec!["ar:ar-2", "ar:ar-1"]);
    assert_eq!((first.next.as_deref(), first.total), (Some("2"), Some(3)));
    let rest = browse(Some(root.items[0].clone()), first.next.clone(), 2);
    assert_eq!(ids(&rest), vec!["ar:ar-3"]);
    assert_eq!(rest.items[0].title, "Ünïcode & Friends");
    assert!(rest.next.is_none());
    assert_eq!(
        server.count("/rest/getArtists.view"),
        1,
        "the second page came from the cache"
    );

    // Artist → albums → tracks.
    let albums = browse(Some(first.items[1].clone()), None, 50);
    assert_eq!(ids(&albums), vec!["al:al-1", "al:al-2"]);
    assert_eq!(albums.items[0].year, Some(2021));
    assert_eq!(albums.items[0].child_count, Some(3));
    let tracks = browse(Some(albums.items[0].clone()), None, 50);
    assert_eq!(ids(&tracks), vec!["tr:tr-1", "tr:tr-2", "tr:tr-3"]);
    let aurora = &tracks.items[0];
    assert_eq!(aurora.title, "Aurora");
    assert_eq!((aurora.track_number, aurora.disc_number), (Some(1), Some(1)));
    assert_eq!(aurora.isrc.as_deref(), Some("XXJF12100001"));
    assert_eq!(aurora.album.as_deref(), Some("Polar Night"));
    assert!(aurora.playable);

    // Album lists, paged by the server.
    let lists = browse(Some(root.items[1].clone()), None, 50);
    assert_eq!(lists.items.len(), 6);
    let newest = lists
        .items
        .iter()
        .find(|i| i.id == "f:albums:newest")
        .unwrap()
        .clone();
    let page1 = browse(Some(newest.clone()), None, 2);
    assert_eq!(ids(&page1), vec!["al:al-3", "al:al-2"]);
    assert_eq!(page1.next.as_deref(), Some("2"));
    let page2 = browse(Some(newest), page1.next, 2);
    assert_eq!(ids(&page2), vec!["al:al-1"]);
    assert!(page2.next.is_none());

    // Playlists.
    let playlists = browse(Some(root.items[2].clone()), None, 50);
    assert_eq!(ids(&playlists), vec!["pl:pl-1", "pl:pl-2"]);
    assert_eq!(playlists.items[0].child_count, Some(2));
    let evening = browse(Some(playlists.items[0].clone()), None, 50);
    assert_eq!(ids(&evening), vec!["tr:tr-2", "tr:tr-6"]);
    assert!(
        browse(Some(playlists.items[1].clone()), None, 50)
            .items
            .is_empty()
    );

    // Starred.
    let starred = browse(Some(root.items[3].clone()), None, 50);
    assert_eq!(
        ids(&starred),
        vec!["f:starred:songs", "f:starred:albums", "f:starred:artists"]
    );
    let starred_ids: Vec<Vec<String>> = starred
        .items
        .iter()
        .map(|folder| ids(&browse(Some(folder.clone()), None, 50)))
        .collect();
    assert_eq!(
        starred_ids,
        vec![vec!["tr:tr-1"], vec!["al:al-3"], vec!["ar:ar-2"]]
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
                .browse(ID, Some(item(ItemKind::Track, "tr:tr-1", "x")), page(5))
        ),
        Err(CallError::Plugin(PluginError::InvalidInput(_)))
    ));
    assert_eq!(
        recv(
            env.host
                .browse(ID, Some(item(ItemKind::Folder, "local:music", "x")), page(5))
        ),
        Err(CallError::Plugin(PluginError::NotFound))
    );
}

#[test]
fn streams_by_quality() {
    let server = Mock::with_api_keys().start();
    let env = env_for(&server);
    sign_in_with_key(&env);
    let resolve = |id: &str, quality| recv(env.host.resolve_stream(ID, track(id, "x"), quality)).unwrap();
    let query = url_query;

    // A FLAC file: the original for lossless, converted for the others.
    let lossless = resolve("tr:tr-1", Quality::Lossless);
    let q = query(&lossless.url);
    assert!(
        lossless
            .url
            .starts_with(&format!("{}/rest/stream.view?", server.url()))
    );
    assert_eq!((q["id"].as_str(), q["format"].as_str()), ("tr-1", "raw"));
    assert!(!q.contains_key("maxBitRate"));
    assert_eq!(q["apiKey"], API_KEY);
    assert_eq!(
        (lossless.codec.as_deref(), lossless.bitrate_kbps),
        (Some("flac"), Some(920))
    );
    assert_eq!(lossless.expires_at_ms, None, "Subsonic stream URLs do not expire");
    assert!(lossless.cacheable);
    let high = resolve("tr:tr-1", Quality::High);
    let q = query(&high.url);
    assert_eq!((q["format"].as_str(), q["maxBitRate"].as_str()), ("mp3", "320"));
    assert_eq!(
        (high.codec.as_deref(), high.bitrate_kbps),
        (Some("mp3"), Some(320))
    );

    // An MP3 at 320 kbit/s: the original up to "high", converted for "low".
    let q = query(&resolve("tr:tr-2", Quality::High).url);
    assert_eq!(q["format"], "raw");
    let low = resolve("tr:tr-2", Quality::Low);
    assert_eq!(query(&low.url)["maxBitRate"], "96");
    assert_eq!(resolve("tr:tr-2", Quality::Normal).bitrate_kbps, Some(192));

    // WMA: the engine cannot play it, so even lossless is converted.
    let wma = resolve("tr:tr-6", Quality::Lossless);
    let q = query(&wma.url);
    assert_eq!((q["format"].as_str(), q["maxBitRate"].as_str()), ("mp3", "320"));

    // The URL works, as the engine would fetch it.
    let fetched = fetch(&lossless.url, &[]).unwrap();
    assert_eq!(fetched.status, 200);
    assert_eq!(fetched.body, AUDIO);
    assert_eq!(fetched.header("content-type"), Some("audio/flac"));

    // A fixed quality and format in the settings win.
    env.settings(
        ID,
        json!({ "server_url": server.url(), "quality": "low", "transcode_format": "opus" }),
    );
    let q = query(&resolve("tr:tr-1", Quality::Lossless).url);
    assert_eq!((q["format"].as_str(), q["maxBitRate"].as_str()), ("opus", "96"));

    assert_eq!(
        recv(
            env.host
                .resolve_stream(ID, track("tr:missing", "x"), Quality::High)
        ),
        Err(CallError::Plugin(PluginError::NotFound))
    );
    assert!(matches!(
        recv(env.host.resolve_stream(ID, track("al:al-1", "x"), Quality::High)),
        Err(CallError::Plugin(PluginError::InvalidInput(_)))
    ));
}

#[test]
fn unplayable_items() {
    let server = Mock::with_api_keys().start();
    let env = env_for(&server);
    sign_in_with_key(&env);
    let tracks = recv(
        env.host
            .browse(ID, Some(item(ItemKind::Album, "al:al-2", "Ice Roads")), page(10)),
    )
    .unwrap();
    let video = &tracks.items[1];
    assert!(tracks.items[0].playable);
    assert!(!video.playable);
    let reason = video.unplayable.as_ref().unwrap();
    assert_eq!(reason.reason, UnplayableReason::Unavailable);
    assert!(reason.message.as_deref().unwrap().contains("Videos"));
    assert!(matches!(
        recv(env.host.resolve_stream(ID, video.clone(), Quality::High)),
        Err(CallError::Plugin(PluginError::Service(_)))
    ));

    // An account without the stream role.
    let server = Mock {
        stream_role: false,
        ..Mock::with_api_keys()
    }
    .start();
    let env = env_for(&server);
    sign_in_with_key(&env);
    let tracks = recv(env.host.browse(
        ID,
        Some(item(ItemKind::Album, "al:al-1", "Polar Night")),
        page(10),
    ))
    .unwrap();
    assert!(tracks.items.iter().all(|t| !t.playable));
    assert_eq!(
        tracks.items[0].unplayable.as_ref().unwrap().reason,
        UnplayableReason::Other
    );
    let error = recv(
        env.host
            .resolve_stream(ID, track("tr:tr-1", "Aurora"), Quality::High),
    )
    .unwrap_err();
    assert!(error.to_string().contains("not allowed to stream"), "{error}");
}

#[test]
fn artwork_by_size() {
    let server = Mock::with_api_keys().start();
    let env = env_for(&server);
    sign_in_with_key(&env);
    let tracks = recv(env.host.browse(
        ID,
        Some(item(ItemKind::Album, "al:al-1", "Polar Night")),
        page(10),
    ))
    .unwrap();
    server.clear();
    let art = recv(env.host.artwork(ID, tracks.items[0].clone(), 512))
        .unwrap()
        .unwrap();
    assert!(server.requests().is_empty(), "the cover id came from the listing");
    let q = url_query(&art.url);
    assert!(
        art.url
            .starts_with(&format!("{}/rest/getCoverArt.view?", server.url()))
    );
    assert_eq!((q["id"].as_str(), q["size"].as_str()), ("al-1_6501f00d", "512"));
    assert_eq!(q["apiKey"], API_KEY);
    assert_eq!(fetch(&art.url, &[]).unwrap().body, PNG);

    // An item the plugin has not listed: one lookup.
    let art = recv(
        env.host
            .artwork(ID, item(ItemKind::Album, "al:al-2", "Ice Roads"), 64),
    )
    .unwrap()
    .unwrap();
    assert!(art.url.contains("id=al-2_6501f00e"));
    assert_eq!(server.count("/rest/getAlbum.view"), 1);
    // No cover, or another source's item.
    assert_eq!(
        recv(env.host.artwork(ID, item(ItemKind::Artist, "ar:ar-3", "x"), 300)).unwrap(),
        None
    );
    assert_eq!(
        recv(env.host.artwork(ID, item(ItemKind::Track, "local:42", "x"), 300)).unwrap(),
        None
    );
}

fn golden(name: &str) -> String {
    std::fs::read_to_string(plugin_dir().join("tests/golden").join(name)).unwrap()
}

#[test]
fn lyrics_synced_unsynced_and_plain() {
    let server = Mock::with_api_keys().start();
    let env = env_for(&server);
    sign_in_with_key(&env);
    let lyrics = |item: Item| recv(env.host.lyrics(ID, item)).unwrap();

    let synced = lyrics(track("tr:tr-1", "Aurora")).unwrap();
    assert_eq!(synced.format, LyricFormat::Lrc);
    assert_eq!(synced.text, golden("synced.lrc"));
    assert_eq!(synced.language.as_deref(), Some("en"));

    let unsynced = lyrics(track("tr:tr-2", "Midnight Sun")).unwrap();
    assert_eq!(unsynced.format, LyricFormat::Plain);
    assert_eq!(unsynced.text, golden("unsynced.txt"));
    assert_eq!(unsynced.language, None, "xxx means unknown");

    // No structured lyrics: plain lyrics found by artist and title.
    let plain = lyrics(track("tr:tr-3", "Snowfall")).unwrap();
    assert_eq!(plain.format, LyricFormat::Plain);
    assert_eq!(plain.text, golden("plain-fallback.txt"));
    assert_eq!(lyrics(track("tr:tr-4", "Glacier")), None);

    // A track from another source is matched by title, artist and length.
    let mut foreign = track("local:/music/midnight-sun.flac", "Midnight Sun");
    foreign.duration_ms = Some(241_000);
    assert_eq!(lyrics(foreign).unwrap().text, golden("unsynced.txt"));

    // A server without OpenSubsonic lyrics still has getLyrics.
    let server = Mock {
        song_lyrics: false,
        ..Mock::with_api_keys()
    }
    .start();
    let env = env_for(&server);
    sign_in_with_key(&env);
    let plain = recv(env.host.lyrics(ID, track("tr:tr-3", "Snowfall")))
        .unwrap()
        .unwrap();
    assert_eq!(plain.text, golden("plain-fallback.txt"));
    assert_eq!(server.count("/rest/getLyricsBySongId.view"), 0);
}

#[test]
fn similar_songs() {
    let server = Mock::with_api_keys().start();
    let env = env_for(&server);
    sign_in_with_key(&env);
    let similar = recv(
        env.host
            .recommendations_for_item(ID, track("tr:tr-1", "Aurora"), page(10)),
    )
    .unwrap();
    assert_eq!(
        ids(&similar),
        vec!["tr:tr-2", "tr:tr-6"],
        "the seed itself is left out"
    );
    assert!(similar.next.is_none());
    let request = &server.requests_to("/rest/getSimilarSongs.view")[0];
    assert_eq!(request.param("count").as_deref(), Some("10"));
    assert_eq!(
        recv(env.host.recommendations_for_user(ID, page(10))),
        Err(CallError::Plugin(PluginError::Unsupported))
    );
}

#[test]
fn logout_forgets_the_sign_in() {
    let server = Mock::with_api_keys().start();
    let env = env_for(&server);
    sign_in_with_key(&env);
    assert!(!env.secrets.keys().is_empty());
    recv(env.host.logout(ID)).unwrap();
    assert!(env.secrets.keys().is_empty());
    assert!(env.host.plugin(ID).unwrap().account.is_none());
    assert_eq!(
        recv(env.host.search(ID, "a", kinds(&[]), page(10))),
        Err(CallError::Plugin(PluginError::AuthRequired))
    );
}

#[test]
fn a_sign_in_belongs_to_its_server() {
    let first = Mock::with_api_keys().start();
    let second = Mock::with_api_keys().start();
    let env = env_for(&first);
    sign_in_with_key(&env);
    env.settings(ID, json!({ "server_url": second.url() }));
    assert_eq!(
        recv(env.host.search(ID, "a", kinds(&[]), page(10))),
        Err(CallError::Plugin(PluginError::AuthRequired))
    );
    for request in second.requests() {
        assert!(
            request.param("apiKey").is_none(),
            "the key went to another server"
        );
    }
    // Back to the first server, the sign-in works again.
    env.settings(ID, json!({ "server_url": first.url() }));
    recv(env.host.search(ID, "a", kinds(&[]), page(10))).unwrap();
}

#[test]
fn signed_in_after_a_restart() {
    let server = Mock::with_api_keys().start();
    let env = env_for(&server);
    sign_in_with_key(&env);
    let host = env.reopen();
    assert_eq!(host.plugin(ID).unwrap().account.as_deref(), Some("alice"));
    let found = recv(host.search(ID, "aurora", kinds(&[ItemKind::Track]), page(5))).unwrap();
    assert_eq!(ids(&found), vec!["tr:tr-1"]);
}

#[test]
fn login_forms_are_described() {
    let server = Mock::navidrome_like().start();
    let env = env_for(&server);
    let flow = login(&env.host, ID, AuthKind::PasswordExchange);
    let AuthStep::AskFields { fields: form } = next_step(&flow) else {
        panic!("expected a form");
    };
    assert_eq!(form[0].kind, FieldKind::Text);
    assert!(form[1].help.as_deref().unwrap().contains("not the password"));
    flow.cancel();
}
