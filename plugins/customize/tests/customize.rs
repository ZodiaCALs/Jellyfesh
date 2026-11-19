//! The Customize plugin in the real host: Pro gating, its settings form in
//! every app language, and the look the host passes on after checking it.

use std::path::Path;

use jf_plugin_host::{
    CallError, Capability, Density, Highlight, HomeSection, LyricsWeight, MiniPlayerStyle, NowPlayingLayout,
    Place, Typeface,
};
use jf_plugin_testkit::*;
use serde_json::{Value, json};

const ID: &str = "org.jellyfesh.customize";

fn env() -> Env {
    let env = Env::new();
    env.install_crate(Path::new(env!("CARGO_MANIFEST_DIR")), "jf-customize");
    env
}

#[test]
fn installs_with_its_settings_in_every_app_language() {
    let env = env();
    let info = env.host.plugin(ID).unwrap();
    assert_eq!(info.name, "Customize");
    assert!(info.permissions.is_empty());
    assert_eq!(info.languages, ["ja", "zh-Hans", "zh-Hant"]);
    assert_eq!(
        recv(env.host.capabilities(ID)).unwrap(),
        vec![Capability::Appearance]
    );
    assert_eq!(info.settings["corners"], 1.0);
    assert_eq!(info.settings["sidebar"].as_array().map(Vec::len), Some(8));
    let localized = |language: &str| -> Value {
        serde_json::from_str(&env.host.localized_settings_schema(ID, language).unwrap()).unwrap()
    };
    let hans = localized("zh-CN");
    assert_eq!(hans["properties"]["corners"]["x-jf-group"], "外观");
    assert_eq!(
        hans["properties"]["sidebar"]["items"]["oneOf"][4]["title"],
        "歌单"
    );
    let hant = localized("zh-Hant-TW");
    assert_eq!(
        hant["properties"]["home"]["items"]["oneOf"][2]["title"],
        "最近播放"
    );
    let ja = localized("ja");
    assert_eq!(ja["properties"]["mini_player"]["title"], "ミニプレーヤー");
    assert_eq!(
        ja["properties"]["now_playing"]["oneOf"][1]["title"],
        "アートワーク"
    );
}

#[test]
fn the_look_needs_pro() {
    let env = env();
    assert_eq!(recv(env.host.appearance_look(ID)), Err(CallError::ProRequired));
    env.host.set_pro(true);
    assert_eq!(
        recv(env.host.appearance_look(ID)).unwrap(),
        jf_plugin_host::Look::default()
    );
}

#[test]
fn settings_become_the_look() {
    let env = env();
    env.host.set_pro(true);
    env.settings(
        ID,
        json!({
            "corners": 0.5, "glass": 0.4, "aurora": 0.75, "typeface": "rounded", "text_size": 1.2,
            "density": "compact", "lyrics_weight": "medium", "lyrics_glow": 0.9,
            "highlight": "custom", "highlight_colour": "#336699",
            "sidebar": ["search", "sources"], "home": ["most-played", "favourites"],
            "now_playing": "artwork", "mini_player": "compact"
        }),
    );
    let look = recv(env.host.appearance_look(ID)).unwrap();
    assert_eq!(look.corner_scale, Some(0.5));
    assert_eq!(look.glass, Some(0.4));
    assert_eq!(look.aurora, None, "0.75 is the default");
    assert_eq!(look.typeface, Some(Typeface::Rounded));
    assert_eq!(look.text_scale, Some(1.2));
    assert_eq!(look.density, Some(Density::Compact));
    assert_eq!(look.lyrics_weight, Some(LyricsWeight::Medium));
    assert_eq!(look.lyrics_glow, Some(0.9));
    assert_eq!(look.lyrics_highlight, Some(Highlight::Custom(0x336699)));
    // The host keeps a place of the library in the sidebar.
    assert_eq!(
        look.sidebar,
        Some(vec![Place::Search, Place::Sources, Place::Albums])
    );
    assert_eq!(
        look.home,
        Some(vec![HomeSection::MostPlayed, HomeSection::Favourites])
    );
    assert_eq!(look.now_playing, Some(NowPlayingLayout::Artwork));
    assert_eq!(look.mini_player, Some(MiniPlayerStyle::Compact));

    // Values the schema does not allow never reach the plugin.
    assert!(
        env.host
            .set_settings(ID, &json!({ "corners": 2.5 }).to_string())
            .is_err()
    );
    assert!(
        env.host
            .set_settings(ID, &json!({ "text_size": 1.23 }).to_string())
            .is_err()
    );
    assert!(
        env.host
            .set_settings(ID, &json!({ "highlight_colour": "orange" }).to_string())
            .is_err()
    );
    assert!(
        env.host
            .set_settings(ID, &json!({ "sidebar": ["home", "home"] }).to_string())
            .is_err()
    );

    // Back to the defaults (Reset to Defaults).
    env.settings(ID, json!({}));
    assert_eq!(
        recv(env.host.appearance_look(ID)).unwrap(),
        jf_plugin_host::Look::default()
    );
}
