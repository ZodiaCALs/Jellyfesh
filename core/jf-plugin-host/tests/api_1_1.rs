//! Plugin API 1.1 (ADR-0018): remote control through the `remote` export,
//! `[links]` in the manifest, and 1.0 plugins in a 1.1 host.

mod common;

use common::*;
use jf_plugin_host::{
    CallError, Capability, HostError, ItemKind, PlaybackMode, PluginError, RemoteDeviceKind, RemoteRepeat,
};

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
    assert_eq!(
        recv(env.host.capabilities(COMPAT)).unwrap(),
        vec![Capability::Search]
    );
    assert!(env.host.plugin(COMPAT).unwrap().links.is_empty());
}
