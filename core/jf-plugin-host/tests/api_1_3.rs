//! Plugin API 1.3 (ADR-0024, ADR-0025): the `library` export (likes and
//! playlists, their page, item and string limits) and the `download` export
//! (refused without the capability, URLs only from the allowlist, a server
//! that refuses), and 1.2 plugins in a 1.3 host.

mod common;

use common::*;
use jf_plugin_host::sanitize::MAX_PAGE;
use jf_plugin_host::{
    CallError, Capability, DownloadQuality, HostError, ItemKind, PageRequest, PluginError, Quality,
};
use serde_json::json;

const LIBRARY: &str = "org.jellyfesh.test.library";
const COMPAT: &str = "org.jellyfesh.test.compat12";

const LIBRARY_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "server_url": { "type": "string", "format": "uri" },
        "hostile": { "type": "boolean", "default": false },
        "refuse": { "type": "boolean", "default": false }
    }
}"#;

fn library_manifest(capabilities: &str, api: &str) -> String {
    format!(
        "id = {LIBRARY:?}\nname = \"Test Library\"\nversion = \"1.0.0\"\napi_version = {api:?}\n\
         author = \"Jellyfesh tests\"\ncapabilities = [{capabilities}]\nplayback_mode = \"stream\"\n\
         \n[network]\nallow = [\"${{settings.server_url}}\"]\n\
         \n[settings]\nschema = \"settings.schema.json\"\n\
         \n[content_policy]\nqualities = [\"high\", \"lossless\"]\n"
    )
}

fn library_env(server: &TestServer, capabilities: &str) -> Env {
    let env = Env::new();
    env.install_with(
        "library",
        &library_manifest(capabilities, "1.3"),
        LIBRARY,
        LIBRARY_SCHEMA,
        &[],
    );
    settings(&env, server, false, false);
    env
}

fn settings(env: &Env, server: &TestServer, hostile: bool, refuse: bool) {
    env.host
        .set_settings(
            LIBRARY,
            &json!({ "server_url": server.url(), "hostile": hostile, "refuse": refuse }).to_string(),
        )
        .unwrap();
}

fn after(cursor: Option<String>, limit: u32) -> PageRequest {
    PageRequest { cursor, limit }
}

#[test]
fn likes_and_playlists_come_in_checked_pages() {
    let server = TestServer::start(|_| Response::new(404, ""));
    let env = library_env(&server, "\"browse\", \"library\"");
    let host = &env.host;
    assert_eq!(
        recv(host.capabilities(LIBRARY)).unwrap(),
        vec![Capability::Browse, Capability::Library]
    );
    let info = host.plugin(LIBRARY).unwrap();
    assert_eq!(
        info.content_policy.qualities,
        vec![Quality::Lossless, Quality::High],
        "the manifest's levels, best first"
    );

    let first = recv(host.library_likes(LIBRARY, ItemKind::Track, page(2))).unwrap();
    let ids: Vec<&str> = first.items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["tr:1", "tr:2"]);
    assert_eq!((first.next.as_deref(), first.total), (Some("2"), Some(5)));
    let rest = recv(host.library_likes(LIBRARY, ItemKind::Track, after(first.next, 50))).unwrap();
    assert_eq!(rest.items.len(), 3);
    assert_eq!(rest.next, None);
    let albums = recv(host.library_likes(LIBRARY, ItemKind::Album, page(50))).unwrap();
    assert!(albums.items.iter().all(|i| i.kind == ItemKind::Album));
    assert_eq!(albums.items.len(), 2);
    assert_eq!(
        recv(host.library_likes(LIBRARY, ItemKind::Artist, page(50))).unwrap_err(),
        CallError::Plugin(PluginError::Unsupported),
        "a kind the service does not keep"
    );
    assert_eq!(
        recv(host.library_likes(LIBRARY, ItemKind::Playlist, page(50))).unwrap_err(),
        CallError::Plugin(PluginError::Unsupported),
        "likes are tracks, albums or artists"
    );

    let lists = recv(host.library_playlists(LIBRARY, page(50))).unwrap();
    let flags: Vec<(&str, bool, bool)> = lists
        .entries
        .iter()
        .map(|e| (e.item.id.as_str(), e.owned, e.songs_listed))
        .collect();
    assert_eq!(
        flags,
        [
            ("pl:mine", true, true),
            ("pl:followed", false, true),
            ("pl:followed-unlisted", false, false)
        ]
    );
    assert_eq!(lists.total, Some(3));
    let two = recv(host.library_playlists(LIBRARY, page(2))).unwrap();
    assert_eq!(two.entries.len(), 2);
    assert_eq!(two.next.as_deref(), Some("2"));
    // A playlist's songs come from browse, page by page.
    let songs = recv(host.browse(LIBRARY, Some(lists.entries[0].item.clone()), page(2))).unwrap();
    assert_eq!(songs.items.len(), 2);
    assert_eq!(songs.next.as_deref(), Some("2"));
}

#[test]
fn hostile_library_answers_are_cut_to_the_limits() {
    let server = TestServer::start(|_| Response::new(404, ""));
    let env = library_env(&server, "\"browse\", \"library\"");
    settings(&env, &server, true, false);
    let host = &env.host;

    let liked = recv(host.library_likes(LIBRARY, ItemKind::Track, page(50))).unwrap();
    assert!(
        liked.items.len() <= 50,
        "never more than asked: {}",
        liked.items.len()
    );
    assert!(
        liked.items.iter().all(|i| i.kind == ItemKind::Track),
        "other kinds dropped"
    );
    assert!(
        liked.items.iter().all(|i| i.id.len() <= 1024),
        "overlong ids dropped"
    );
    assert!(liked.items.iter().all(|i| i.title.len() <= 1024), "titles cut");
    assert_eq!(liked.next, None, "an overlong cursor is dropped");
    let most = recv(host.library_likes(LIBRARY, ItemKind::Track, page(10_000))).unwrap();
    assert!(most.items.len() <= MAX_PAGE as usize);

    let lists = recv(host.library_playlists(LIBRARY, page(20))).unwrap();
    assert!(lists.entries.len() <= 20);
    assert!(lists.entries.iter().all(|e| e.item.kind == ItemKind::Playlist));
    assert!(lists.entries.iter().all(|e| e.item.title.len() <= 1024));
    assert!(
        recv(host.library_playlists(LIBRARY, page(10_000)))
            .unwrap()
            .entries
            .len()
            <= MAX_PAGE as usize
    );
    assert!(
        host.plugin(LIBRARY).unwrap().enabled,
        "a cut answer is no violation"
    );
}

#[test]
fn downloads_need_the_capability_and_an_allowlisted_url() {
    let server = TestServer::start(|_| Response::new(404, ""));
    let env = library_env(&server, "\"browse\", \"library\", \"download\"");
    let host = &env.host;
    let song = track("tr:1", "Invented Song 1");

    let original = recv(host.download(LIBRARY, song.clone(), DownloadQuality::Original)).unwrap();
    assert_eq!(original.url, format!("{}/files/tr-1.flac", server.url()));
    assert_eq!(original.container, "flac");
    assert!(original.original);
    assert_eq!(original.headers[0].name, "x-token");
    assert_eq!(original.file_name.as_deref(), Some("tr-1.flac"));
    let converted = recv(host.download(LIBRARY, song.clone(), DownloadQuality::High)).unwrap();
    assert_eq!(
        (converted.container.as_str(), converted.original),
        ("opus", false)
    );

    // The server does not let the account download: the plugin says so.
    settings(&env, &server, false, true);
    assert_eq!(
        recv(host.download(LIBRARY, song.clone(), DownloadQuality::Original)).unwrap_err(),
        CallError::Plugin(PluginError::Unsupported)
    );
    // A URL off the allowlist never reaches the downloader.
    settings(&env, &server, true, false);
    assert!(matches!(
        recv(host.download(LIBRARY, song.clone(), DownloadQuality::Original)),
        Err(CallError::InvalidResponse(_))
    ));

    // Without the capability, the host refuses before the plugin is asked.
    let without = library_env(&server, "\"browse\", \"library\"");
    assert_eq!(
        recv(
            without
                .host
                .download(LIBRARY, song.clone(), DownloadQuality::Original)
        )
        .unwrap_err(),
        CallError::NotDeclared("download".into())
    );
    let neither = library_env(&server, "\"browse\"");
    assert_eq!(
        recv(neither.host.library_likes(LIBRARY, ItemKind::Track, page(5))).unwrap_err(),
        CallError::NotDeclared("library".into())
    );
    assert_eq!(
        recv(neither.host.library_playlists(LIBRARY, page(5))).unwrap_err(),
        CallError::NotDeclared("library".into())
    );
}

#[test]
fn api_1_3_capabilities_need_an_api_1_3_manifest() {
    let env = Env::new();
    let path = package_with(
        &env.incoming(),
        "old.jfpkg",
        &library_manifest("\"browse\", \"download\"", "1.2")
            .replace("[content_policy]\nqualities = [\"high\", \"lossless\"]\n", ""),
        component("library"),
        LIBRARY_SCHEMA,
        &[],
    );
    let error = env.host.install(&path).unwrap_err();
    assert!(
        matches!(&error, HostError::Manifest(text) if text.contains("needs api_version 1.3")),
        "{error}"
    );
}

#[test]
fn a_plugin_built_for_1_2_runs_in_a_1_3_host() {
    let env = Env::new();
    let manifest = format!(
        "id = {COMPAT:?}\nname = \"Compat 1.2\"\nversion = \"1.0.0\"\napi_version = \"1.2\"\n\
         author = \"Jellyfesh tests\"\ncapabilities = [\"search\", \"discover\"]\nplayback_mode = \"stream\"\n\
         \n[settings]\nschema = \"settings.schema.json\"\n"
    );
    env.install("compat-1-2", &manifest, COMPAT, None);
    assert_eq!(titles(&env.host, COMPAT, "hello"), ["1.2 says hello"]);
    let shelves = recv(env.host.discover(COMPAT)).unwrap();
    assert_eq!(shelves[0].title, "Compat Shelf");
    assert_eq!(
        recv(env.host.capabilities(COMPAT)).unwrap(),
        vec![Capability::Search, Capability::Discover]
    );
}
