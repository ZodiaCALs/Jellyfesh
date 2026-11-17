//! Plugin API 1.1 (ADR-0018): remote control through the `remote` export,
//! `[links]` in the manifest, RSA signing with a key from the keystore
//! (`crypto.rsa-sign-pkcs1-sha256`), a fixed oauth redirect port
//! (`[oauth] redirect_port`), and 1.0 plugins in a 1.1 host.

mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use common::*;
use jf_plugin_host::{
    AuthKind, AuthStep, CallError, Capability, HostError, ItemKind, PlaybackMode, PluginError,
    RemoteDeviceKind, RemoteRepeat, SecretStore,
};
use rsa::pkcs8::{EncodePrivateKey, LineEnding};

const REMOTE: &str = "org.jellyfesh.test.remote";
const COMPAT: &str = "org.jellyfesh.test.compat";

fn remote_manifest(api: &str, links: bool) -> String {
    let mut text = format!(
        "id = {REMOTE:?}\nname = \"Test Remote\"\nversion = \"1.0.0\"\napi_version = {api:?}\n\
         author = \"Jellyfesh tests\"\ncapabilities = [\"search\", \"remote\"]\nplayback_mode = \"remote\"\n\
         \n[settings]\nschema = \"settings.schema.json\"\n"
    );
    if links {
        text += "\n[links]\ntrack = \"https://music.example.com/song/{id}\"\n\
                 album = \"https://music.example.com/album?id={id}\"\n";
    }
    text
}

fn remote_env() -> Env {
    let env = Env::new();
    env.install("remote", &remote_manifest("1.1", true), REMOTE, None);
    env
}

#[test]
fn a_remote_source_lists_devices_and_follows_commands() {
    let env = remote_env();
    let host = &env.host;
    let info = host.plugin(REMOTE).unwrap();
    assert_eq!(info.playback_mode, PlaybackMode::Remote);
    assert!(info.capabilities.contains(&Capability::Remote));
    assert_eq!(
        recv(host.capabilities(REMOTE)).unwrap(),
        vec![Capability::Search, Capability::Remote],
        "remote is listed although meta.capabilities cannot report it"
    );

    // The nameless device is dropped and the phone's 250 % becomes 100 %.
    let devices = recv(host.remote_devices(REMOTE)).unwrap();
    let names: Vec<_> = devices.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["Desk Speaker", "Phone"]);
    assert_eq!(devices[0].kind, RemoteDeviceKind::Speaker);
    assert_eq!(devices[1].volume_percent, Some(100));
    assert!(devices.iter().all(|d| !d.active));
    assert_eq!(
        recv(host.remote_state(REMOTE)).unwrap(),
        None,
        "nothing plays yet"
    );

    let tracks = vec![track("a", "First"), track("b", "Second")];
    recv(host.remote_play(REMOTE, None, None, tracks, 1)).unwrap();
    let state = recv(host.remote_state(REMOTE)).unwrap().expect("playing");
    assert!(state.playing);
    assert_eq!(state.item.as_ref().map(|i| i.title.as_str()), Some("Second"));
    assert_eq!(state.device.as_ref().map(|d| d.id.as_str()), Some("desk"));
    assert!(state.device.as_ref().unwrap().active);

    recv(host.remote_pause(REMOTE)).unwrap();
    recv(host.remote_previous(REMOTE)).unwrap();
    recv(host.remote_set_shuffle(REMOTE, true)).unwrap();
    recv(host.remote_set_repeat(REMOTE, RemoteRepeat::One)).unwrap();
    // The host cuts the volume to 100 before the plugin (which refuses more) sees it.
    recv(host.remote_set_volume(REMOTE, 150)).unwrap();
    // A seek past the end: the plugin believes it, the host does not.
    recv(host.remote_seek(REMOTE, 999_999)).unwrap();
    let state = recv(host.remote_state(REMOTE)).unwrap().unwrap();
    assert!(!state.playing);
    assert!(state.shuffle);
    assert_eq!(state.repeat, RemoteRepeat::One);
    assert_eq!(state.item.as_ref().map(|i| i.id.as_str()), Some("a"));
    assert_eq!(state.position_ms, 200_000, "the position stays within the song");
    assert_eq!(state.device.as_ref().and_then(|d| d.volume_percent), Some(100));

    recv(host.remote_transfer(REMOTE, "phone".into(), true)).unwrap();
    recv(host.remote_next(REMOTE)).unwrap();
    let state = recv(host.remote_state(REMOTE)).unwrap().unwrap();
    assert!(state.playing);
    assert_eq!(state.device.as_ref().map(|d| d.name.as_str()), Some("Phone"));
    assert_eq!(state.item.as_ref().map(|i| i.id.as_str()), Some("b"));
    let devices = recv(host.remote_devices(REMOTE)).unwrap();
    assert_eq!(
        devices
            .iter()
            .filter(|d| d.active)
            .map(|d| d.id.as_str())
            .collect::<Vec<_>>(),
        ["phone"]
    );

    // A container plays from its own tracks; the plugin's errors come through.
    let mut album = track("al", "An Album");
    album.kind = ItemKind::Album;
    recv(host.remote_play(REMOTE, Some("desk".into()), Some(album), Vec::new(), 0)).unwrap();
    let state = recv(host.remote_state(REMOTE)).unwrap().unwrap();
    assert_eq!(state.context.as_ref().map(|c| c.id.as_str()), Some("al"));
    assert_eq!(state.item.as_ref().map(|i| i.id.as_str()), Some("al-1"));
    // A container from its second track: the index reaches the plugin as given.
    let mut album = track("al", "An Album");
    album.kind = ItemKind::Album;
    recv(host.remote_play(REMOTE, Some("desk".into()), Some(album), Vec::new(), 1)).unwrap();
    let state = recv(host.remote_state(REMOTE)).unwrap().unwrap();
    assert_eq!(state.item.as_ref().map(|i| i.id.as_str()), Some("al-2"));
    assert!(matches!(
        recv(host.remote_transfer(REMOTE, "nowhere".into(), true)),
        Err(CallError::Plugin(PluginError::NotFound))
    ));
    assert!(matches!(
        recv(host.remote_play(REMOTE, None, None, Vec::new(), 0)),
        Err(CallError::Plugin(PluginError::InvalidInput(_)))
    ));
    assert!(recv(host.remote_transfer(REMOTE, String::new(), true)).is_err());
}

#[test]
fn links_open_items_in_the_service() {
    let env = remote_env();
    let links = env.host.plugin(REMOTE).unwrap().links;
    assert_eq!(
        links.url_for(ItemKind::Track, "a b").as_deref(),
        Some("https://music.example.com/song/a%20b")
    );
    assert_eq!(
        links.url_for(ItemKind::Album, "42").as_deref(),
        Some("https://music.example.com/album?id=42")
    );
    assert_eq!(links.url_for(ItemKind::Artist, "1"), None);
}

#[test]
fn remote_calls_need_the_remote_capability() {
    let env = Env::new();
    let server = TestServer::start(|_| Response::new(404, ""));
    env.install("echo", &echo_manifest(), ECHO, Some(&server.url()));
    assert!(matches!(
        recv(env.host.remote_devices(ECHO)),
        Err(CallError::NotDeclared(what)) if what == "remote"
    ));
    assert!(matches!(
        recv(env.host.remote_pause(ECHO)),
        Err(CallError::NotDeclared(_))
    ));
}

#[test]
fn a_1_1_manifest_is_refused_where_its_features_are_missing() {
    let env = Env::new();
    // `remote` needs api_version 1.1.
    let path = package(
        &env.incoming(),
        "old.jfpkg",
        &remote_manifest("1.0", false),
        component("remote"),
        true,
    );
    assert!(matches!(env.host.install(&path), Err(HostError::Manifest(_))));

    // A component without the `remote` export (built for 1.0) cannot be a remote source.
    let path = package(
        &env.incoming(),
        "no-remote.jfpkg",
        &remote_manifest("1.1", false),
        component("compat-1-0"),
        true,
    );
    match env.host.install(&path) {
        Err(HostError::Incompatible(why)) => assert!(why.contains("declares remote"), "{why}"),
        other => panic!("expected an incompatible component, got {other:?}"),
    }
}

#[test]
fn a_plugin_built_for_1_0_runs_in_a_1_1_host() {
    let env = Env::new();
    let manifest = manifest(COMPAT, &["search"], &[], &[], 5);
    assert!(manifest.contains("api_version = \"1.0\""));
    env.install("compat-1-0", &manifest, COMPAT, None);
    assert_eq!(titles(&env.host, COMPAT, "hello"), ["1.0 says hello"]);
    // Its 1.0 `crypto` import links although 1.1 added rsa-sign-pkcs1-sha256.
    let found = recv(env.host.search(COMPAT, "hello", any(), page(5))).unwrap();
    assert_eq!(
        found.items[0].artist.as_deref(),
        Some("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"),
        "SHA-256 of \"hello\" from the 1.0 crypto import"
    );
    assert_eq!(
        recv(env.host.capabilities(COMPAT)).unwrap(),
        vec![Capability::Search]
    );
    assert!(env.host.plugin(COMPAT).unwrap().links.is_empty());
}

/// The echo plugin with an API 1.1 manifest, extended by `extra` (tables).
fn echo_1_1(extra: &str) -> String {
    let manifest = echo_manifest().replace("api_version = \"1.0\"", "api_version = \"1.1\"");
    assert!(manifest.contains("api_version = \"1.1\""));
    format!("{manifest}{extra}")
}

/// The signature the echo plugin's `do:rsa-sign:<secret>` search returns.
fn sign_with(env: &Env, secret: &str) -> String {
    titles(&env.host, ECHO, &format!("do:rsa-sign:{secret}")).remove(0)
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn plugins_sign_with_a_key_the_host_reads_from_their_own_keystore() {
    let env = Env::new();
    env.install("echo", &echo_1_1(""), ECHO, None);
    let private = rsa::PrivateKey::new(&mut getrandom::SysRng, 2048).unwrap();
    let pem = private.to_pkcs8_pem(LineEnding::LF).unwrap();
    env.secrets
        .set(&format!("jellyfesh/{ECHO}/signing-key"), &pem)
        .unwrap();

    // A good key: a PKCS #1 v1.5 SHA-256 signature the public key verifies.
    let answer = sign_with(&env, "signing-key");
    let signature = unhex(answer.strip_prefix("ok:").unwrap_or_else(|| panic!("{answer}")));
    assert_eq!(signature.len(), 256);
    private
        .to_public_key()
        .pkcs1v15_verify::<sha2::Sha256>(b"jellyfesh", &signature)
        .unwrap();

    // No such secret, a name outside the plugin's namespace, a secret that is
    // not a key (and the error does not quote it).
    assert_eq!(
        sign_with(&env, "missing"),
        "err:CryptoError::InvalidKey(\"there is no secret named \\\"missing\\\"\")"
    );
    env.secrets
        .set("jellyfesh/org.jellyfesh.test.other/signing-key", &pem)
        .unwrap();
    let outside = sign_with(&env, "../org.jellyfesh.test.other/signing-key");
    assert!(
        outside.starts_with("err:CryptoError::InvalidKey(\"secret names"),
        "{outside}"
    );
    env.secrets
        .set(&format!("jellyfesh/{ECHO}/bad-key"), "hunter2-not-a-key")
        .unwrap();
    let bad = sign_with(&env, "bad-key");
    assert!(
        bad.starts_with("err:CryptoError::InvalidKey(\"not an RSA private key"),
        "{bad}"
    );
    assert!(!bad.contains("hunter2"), "{bad}");
    // The plugin stays healthy after refused calls.
    assert!(sign_with(&env, "signing-key").starts_with("ok:"));
}

#[test]
fn signing_refuses_more_than_16_mib() {
    // The debug-built test plugin needs more than the production CPU budget
    // to make 16 MiB; the limit under test is the host's.
    let env = Env::with_limits(jf_plugin_host::Limits {
        cpu: Duration::from_secs(5),
        ..test_limits()
    });
    env.install("echo", &echo_1_1(""), ECHO, None);
    let private = rsa::PrivateKey::new(&mut getrandom::SysRng, 1024).unwrap();
    env.secrets
        .set(
            &format!("jellyfesh/{ECHO}/signing-key"),
            &private.to_pkcs8_pem(LineEnding::LF).unwrap(),
        )
        .unwrap();
    assert_eq!(
        titles(&env.host, ECHO, "do:rsa-sign-big"),
        ["err:CryptoError::TooLarge"]
    );
}

/// A raw `GET` to the loopback listener, as the browser would send it.
fn browser_get(url: &str) -> String {
    let url = url::Url::parse(url).unwrap();
    let mut stream = TcpStream::connect((url.host_str().unwrap(), url.port().unwrap())).unwrap();
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

fn next(flow: &jf_plugin_host::AuthFlow) -> AuthStep {
    flow.steps()
        .recv_timeout(Duration::from_secs(10))
        .expect("no login step within 10 s")
}

/// A local port nothing listens on (free a moment ago).
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn an_oauth_login_comes_back_to_the_manifests_fixed_port() {
    let server = TestServer::start(|request| match request.path() {
        "/token" if request.form("code").as_deref() == Some("good-code") => {
            Response::json(serde_json::json!({ "access_token": "at-1", "name": "OAuth User" }))
        }
        _ => Response::new(404, "not found"),
    });
    let port = free_port();
    let env = Env::new();
    env.install(
        "echo",
        &echo_1_1(&format!("\n[oauth]\nredirect_port = {port}\n")),
        ECHO,
        Some(&server.url()),
    );
    let flow = env.host.begin_auth(ECHO, AuthKind::Oauth).unwrap();
    let AuthStep::OpenUrl { url } = next(&flow) else {
        panic!("expected a browser page");
    };
    let url = url::Url::parse(&url).unwrap();
    let param = |key: &str| url.query_pairs().find(|(k, _)| k == key).unwrap().1.into_owned();
    let redirect_uri = param("redirect_uri");
    assert_eq!(redirect_uri, format!("http://127.0.0.1:{port}/callback"));
    let answer = browser_get(&format!("{redirect_uri}?code=good-code&state={}", param("state")));
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    assert_eq!(
        next(&flow),
        AuthStep::Done {
            account: "OAuth User".into()
        }
    );
    let token = server
        .requests()
        .into_iter()
        .find(|r| r.path() == "/token")
        .unwrap();
    assert_eq!(token.form("redirect_uri").unwrap(), redirect_uri);
    // The port is free again once the login is over.
    drop(flow);
    std::thread::sleep(Duration::from_millis(100));
    assert!(TcpListener::bind(("127.0.0.1", port)).is_ok());
}

#[test]
fn a_busy_fixed_port_ends_the_login_with_a_message_that_names_it() {
    let busy = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = busy.local_addr().unwrap().port();
    let env = Env::new();
    env.install(
        "echo",
        &echo_1_1(&format!("\n[oauth]\nredirect_port = {port}\n")),
        ECHO,
        None,
    );
    let flow = env.host.begin_auth(ECHO, AuthKind::Oauth).unwrap();
    match next(&flow) {
        AuthStep::Failed { message } => {
            assert!(message.contains(&format!("Port {port}")), "{message}");
            assert!(
                message.contains(&format!("http://127.0.0.1:{port}/callback")),
                "{message}"
            );
        }
        other => panic!("expected the login to fail, got {other:?}"),
    }
    drop(busy);
}
