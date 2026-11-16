//! Open in {Service} (API 1.1): the page asks the source for an item's link, filled in from the
//! manifest's `[links]` templates as `jf_plugin_model::links` fills them in for the apps.

#![cfg(not(target_arch = "wasm32"))]

use jf_web::Source;

const RULES: &str = r#"[{"Setting":"server_url"}]"#;
const SETTINGS: &str = r#"{"server_url":"http://192.168.1.20:4533"}"#;
const POLICY: &str = r#"{"unified_search":true,"mixed_queue":true,"library_storage":"Full","metadata_ttl_secs":null,"lyrics_sync":true,"attribution":null}"#;

fn source(links: Option<&str>) -> Source {
    Source::new(
        "org.example.linked",
        "Linked Sample",
        RULES,
        SETTINGS,
        POLICY,
        links.map(str::to_owned),
    )
    .unwrap()
}

#[test]
fn items_link_to_their_service() {
    // As jf_web_sandbox::PackageInfo writes the manifest's [links].
    let linked = source(Some(
        r#"{"track":"https://music.example.com/song/{id}","album":"https://music.example.com/album?id={id}","artist":null,"playlist":null}"#,
    ));
    assert_eq!(
        linked.link("Track", "42 b/c").as_deref(),
        Some("https://music.example.com/song/42%20b%2Fc")
    );
    assert_eq!(
        linked.link("Album", "7").as_deref(),
        Some("https://music.example.com/album?id=7")
    );
    assert_eq!(linked.link("Artist", "1"), None);
    assert_eq!(linked.link("Folder", "1"), None);
    assert_eq!(linked.link("Song", "1"), None, "not an item kind");
}

#[test]
fn plugins_without_links_have_none() {
    for links in [None, Some("null")] {
        assert_eq!(source(links).link("Track", "1"), None);
    }
}
