//! Jellyfesh Pro (licence keys from a key pair made in the test, store entitlements, Preview,
//! store builds), the look from an appearance plugin, the update check against a local server,
//! and resuming playback on launch. The plugins, releases and keys are made up here.

mod common;

use std::time::Duration;

use common::*;
use jf_core::{
    Command, Config, Distribution, Event, LicenceError, ProConfig, ProSource, Settings, Status, Store,
    StoreEntitlement, Typeface,
};
use serde_json::json;

const LOOK: &str = "org.jellyfesh.test.look";

const LOOK_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "look": { "type": "string", "maxLength": 4096, "default": "{}" },
        "nan_glass": { "type": "boolean", "default": false }
    }
}"#;

fn look_manifest() -> String {
    format!(
        "id = {LOOK:?}\nname = \"Test Look\"\nversion = \"1.0.0\"\napi_version = \"1.2\"\n\
         author = \"Jellyfesh tests\"\ncapabilities = [\"appearance\"]\nplayback_mode = \"stream\"\n\
         \n[settings]\nschema = \"settings.schema.json\"\n"
    )
}

/// A key pair made for the test and a key it signed (issued on day 20409).
fn licence() -> (jf_licence::PublicKey, String) {
    let (secret, public) = jf_licence::generate_keypair_with_id(3);
    let key = jf_licence::issue(&secret, 3, &jf_licence::new_licence(20_409));
    (public, key)
}

/// A direct download that knows `public` and keeps its session.
fn direct(public: jf_licence::PublicKey) -> impl Fn(&mut Config) {
    move |c: &mut Config| {
        c.distribution = Distribution::Direct;
        c.pro = ProConfig {
            preview: false,
            licence_keys: vec![public],
        };
        c.persist_session = true;
        c.restore_session = true;
        // No update check against GitHub from the tests.
        c.update_url = "http://127.0.0.1:9/releases".into();
    }
}

#[test]
fn licence_keys_turn_pro_on_and_are_kept() {
    let (public, key) = licence();
    let env = Env::with(direct(public));
    let pro = env.core.snapshot().pro;
    assert!(!pro.active);
    assert!(pro.licence_keys, "direct downloads take licence keys");
    assert!(pro.stores.is_empty());

    let error = env.fails(Command::ActivateLicence {
        key: "JFP1-NOT-A-KEY".into(),
    });
    assert!(error.contains("does not look like"), "{error}");
    assert_eq!(
        env.core.snapshot().pro.licence_error,
        Some(LicenceError::Malformed)
    );
    // A key another key pair signed.
    let (other, _) = jf_licence::generate_keypair_with_id(3);
    let forged = jf_licence::issue(&other, 3, &jf_licence::new_licence(20_409));
    env.fails(Command::ActivateLicence { key: forged });
    assert_eq!(
        env.core.snapshot().pro.licence_error,
        Some(LicenceError::BadSignature)
    );

    // The real one, typed in lower case with spaces.
    let typed = key.to_ascii_lowercase().replace('-', " ");
    env.run(
        Command::ActivateLicence { key: typed },
        |e| matches!(e, Event::Pro(p) if p.active),
    );
    let pro = env.core.snapshot().pro;
    assert_eq!(pro.licence_error, None);
    match &pro.source {
        ProSource::Licence {
            id_suffix,
            issued_at_ms,
        } => {
            assert_eq!(id_suffix.len(), 4);
            assert_eq!(*issued_at_ms, 20_409 * 86_400_000);
        }
        other => panic!("{other:?}"),
    }

    // Kept with the settings, and checked again at start.
    let env = env.restart_with(direct(public));
    assert!(env.core.snapshot().pro.active);
    // A build that no longer knows the key: Pro is off and says why.
    let (_, stranger) = jf_licence::generate_keypair_with_id(9);
    let env = env.restart_with(direct(stranger));
    let pro = env.core.snapshot().pro;
    assert!(!pro.active);
    assert_eq!(pro.licence_error, Some(LicenceError::UnknownKey));

    let env = env.restart_with(direct(public));
    env.run(
        Command::RemoveLicence,
        |e| matches!(e, Event::Pro(p) if !p.active),
    );
    let env = env.restart_with(direct(public));
    assert!(!env.core.snapshot().pro.active, "removed for good");
}

#[test]
fn store_builds_take_pro_from_their_store_only() {
    let (public, key) = licence();
    let env = Env::with(|c| {
        c.distribution = Distribution::AppStore;
        c.pro = ProConfig {
            preview: false,
            licence_keys: vec![public],
        };
    });
    let snapshot = env.core.snapshot();
    assert!(!snapshot.pro.licence_keys);
    assert_eq!(snapshot.pro.stores, vec![Store::AppStore]);
    assert!(!snapshot.sources.can_install_plugins);
    let error = env.fails(Command::ActivateLicence { key });
    assert!(error.contains("store"), "{error}");
    assert!(env.fails(Command::CheckForUpdates).contains("store"));

    // Another store's entitlement and another product are ignored; this
    // store's turns Pro on.
    let entitlement = |store, product: &str, active| StoreEntitlement {
        store,
        product: product.into(),
        active,
        transaction: None,
    };
    env.core
        .set_store_entitlement(entitlement(Store::GooglePlay, "pro", true));
    env.core
        .set_store_entitlement(entitlement(Store::AppStore, "org.example.other", true));
    env.core
        .set_store_entitlement(entitlement(Store::AppStore, "org.jellyfesh.pro", true));
    let events = wait_for(
        &env.events,
        Duration::from_secs(10),
        |e| matches!(e, Event::Pro(p) if p.active),
    );
    assert_eq!(
        events.iter().filter(|e| matches!(e, Event::Pro(_))).count(),
        1,
        "the ignored ones changed nothing"
    );
    assert_eq!(
        env.core.snapshot().pro.source,
        ProSource::Store {
            store: Store::AppStore
        }
    );
    // A refund.
    env.core
        .set_store_entitlement(entitlement(Store::AppStore, "org.jellyfesh.pro", false));
    wait_for(
        &env.events,
        Duration::from_secs(10),
        |e| matches!(e, Event::Pro(p) if !p.active),
    );

    // Community plugins stay off in store builds.
    let settings = Settings {
        allow_community_plugins: true,
        ..(*env.core.snapshot().settings).clone()
    };
    env.run(Command::SetSettings { settings }, |e| {
        matches!(e, Event::Settings(_))
    });
    assert!(!env.core.snapshot().settings.allow_community_plugins);
    assert!(
        env.fails(Command::AcknowledgeUnofficial { id: "org.x".into() })
            .contains("not available")
    );
}

#[test]
fn preview_builds_have_pro_for_everyone() {
    let env = Env::with(|c| {
        c.pro = ProConfig {
            preview: true,
            licence_keys: Vec::new(),
        };
    });
    let pro = env.core.snapshot().pro;
    assert!(pro.active && pro.preview);
    assert_eq!(pro.source, ProSource::Preview);
    // Builds from source know no release keys unless they were given some.
    if option_env!("JF_LICENCE_PUBLIC_KEYS").is_none_or(str::is_empty) {
        assert!(ProConfig::for_this_build().licence_keys.is_empty());
    }
}

#[test]
fn the_look_comes_from_the_appearance_plugin_while_pro_is_active() {
    let (public, key) = licence();
    let env = Env::with(|c| {
        c.pro = ProConfig {
            preview: false,
            licence_keys: vec![public],
        };
    });
    env.install_with_schema("appearance", &look_manifest(), LOOK, LOOK_SCHEMA);
    let look = json!({ "corner_scale": 1.5, "typeface": "Serif", "sidebar": ["Songs", "Home"] });
    env.settings(LOOK, json!({ "look": look.to_string() }));
    assert!(
        !env.core.snapshot().look.customized,
        "without Pro the defaults stay"
    );

    env.run(
        Command::ActivateLicence { key },
        |e| matches!(e, Event::Look(l) if l.customized),
    );
    let look = env.core.snapshot().look;
    assert_eq!(look.corner_scale, 1.5);
    assert_eq!(look.typeface, Typeface::Serif);
    assert_eq!(look.plugin.as_deref(), Some(LOOK));
    assert_eq!(
        look.home,
        jf_core::LookSnapshot::defaults(0).home,
        "what the plugin leaves out keeps its default"
    );

    // New settings, a new look.
    let smaller = json!({ "corner_scale": 0.5 });
    let id = env.core.send(Command::SetPluginSettings {
        id: LOOK.into(),
        settings_json: json!({ "look": smaller.to_string() }).to_string(),
    });
    wait_for(&env.events, Duration::from_secs(20), |e| {
        assert!(!matches!(e, Event::CommandFailed { id: failed, .. } if *failed == id));
        matches!(e, Event::Look(l) if l.corner_scale == 0.5)
    });
    // Off, and on again.
    env.run(
        Command::SetPluginEnabled {
            id: LOOK.into(),
            enabled: false,
        },
        |e| matches!(e, Event::Look(l) if !l.customized),
    );
    env.run(
        Command::SetPluginEnabled {
            id: LOOK.into(),
            enabled: true,
        },
        |e| matches!(e, Event::Look(l) if l.customized),
    );
    // Without Pro, back to the defaults.
    env.run(
        Command::RemoveLicence,
        |e| matches!(e, Event::Look(l) if !l.customized),
    );
    assert_eq!(env.core.snapshot().look.plugin, None);
}

/// A releases list as GitHub's API gives it, with invented releases.
fn releases() -> serde_json::Value {
    let release = |tag: &str, draft: bool| {
        json!({
            "tag_name": tag,
            "html_url": format!("https://github.com/ZodiaCALs/Jellyfesh/releases/tag/{tag}"),
            "draft": draft,
            "prerelease": tag.contains('-'),
        })
    };
    json!([
        release("v99.1.0", true),
        release("v99.0.0-beta.1", false),
        release("v0.0.1", false)
    ])
}

#[test]
fn direct_downloads_check_for_updates() {
    let server = TestServer::start(|req| match req.path() {
        "/releases" => Response::json(releases()),
        _ => Response::new(404, ""),
    });
    let url = format!("{}/releases", server.url());
    let setup = |url: String| {
        move |c: &mut Config| {
            c.distribution = Distribution::Direct;
            c.update_url = url;
        }
    };
    let env = Env::with(setup(url.clone()));
    // The setting is on and nothing was checked yet: a check at start.
    assert!(until(Duration::from_secs(30), || {
        let update = env.core.snapshot().update;
        update.available.is_some() && !update.checking
    }));
    let update = env.core.snapshot().update;
    assert!(update.supported && update.auto && !update.checking);
    let available = update.available.clone().unwrap();
    assert_eq!(available.version, "99.0.0-beta.1", "drafts never count");
    assert!(available.url.ends_with("/v99.0.0-beta.1"));
    assert!(update.last_checked_ms.is_some());
    assert_eq!(server.count("/releases"), 1);

    // Check for Updates asks again at once.
    env.run(Command::CheckForUpdates, |e| {
        matches!(e, Event::Update(u) if !u.checking && u.last_checked_ms.is_some() && u.version > update.version + 1)
    });
    assert_eq!(server.count("/releases"), 2);

    // The setting off: no checks by themselves.
    let settings = Settings {
        check_for_updates: false,
        ..(*env.core.snapshot().settings).clone()
    };
    env.run(
        Command::SetSettings { settings },
        |e| matches!(e, Event::Update(u) if !u.auto),
    );

    // A restart within the day does not ask again; what was found is kept.
    let env = env.restart_with(setup(url.clone()));
    assert!(until(Duration::from_secs(10), || {
        env.core.snapshot().update.available.is_some()
    }));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(server.count("/releases"), 2);

    // Other builds do not check at all.
    let env = env.restart_with(|c| c.distribution = Distribution::Source);
    assert!(env.fails(Command::CheckForUpdates).contains("package manager"));
    assert!(until(Duration::from_secs(10), || {
        let update = env.core.snapshot().update;
        update.version > 0 && !update.supported && !update.auto
    }));
    assert_eq!(server.count("/releases"), 2);
}

#[test]
fn a_failed_check_says_why_and_counts_as_a_check() {
    let server = TestServer::start(|_| Response::new(500, "invented failure"));
    let url = format!("{}/releases", server.url());
    let env = Env::with(|c| {
        c.distribution = Distribution::Direct;
        c.update_url = url;
    });
    assert!(until(Duration::from_secs(30), || {
        let update = env.core.snapshot().update;
        update.error.is_some() && !update.checking
    }));
    let update = env.core.snapshot().update;
    assert!(update.error.as_deref().unwrap().contains("500"));
    assert!(update.available.is_none());
    assert!(update.last_checked_ms.is_some());
}

/// Played in real time, with the session kept.
fn kept(c: &mut Config) {
    c.persist_session = true;
    c.restore_session = true;
    c.update_url = "http://127.0.0.1:9/releases".into();
    if let jf_core::OutputKind::WavFile { realtime, .. } = &mut c.output {
        *realtime = true;
    }
}

#[test]
fn playback_resumes_on_launch_when_asked() {
    let env = Env::with(kept);
    let music = env.dir.path().join("music");
    std::fs::create_dir_all(&music).unwrap();
    let song = music.join("invented-tone.wav");
    sine_wav(&song, 440.0, 30.0);
    env.run(
        Command::PlayTracks {
            tracks: vec![jf_core::TrackRef::File(song.clone())],
            start: 0,
        },
        |e| matches!(e, Event::Playback(p) if p.status == Status::Playing),
    );
    let settings = Settings {
        resume_on_launch: true,
        ..(*env.core.snapshot().settings).clone()
    };
    env.run(Command::SetSettings { settings }, |e| {
        matches!(e, Event::Settings(_))
    });
    env.run(
        Command::Pause,
        |e| matches!(e, Event::Playback(p) if p.status == Status::Paused),
    );
    let env = env.restart_with(kept);
    assert!(until(Duration::from_secs(20), || {
        let snapshot = env.core.snapshot();
        snapshot.playback.status == Status::Playing && snapshot.now_playing.is_some()
    }));
    let now = env
        .core
        .snapshot()
        .now_playing
        .clone()
        .expect("the restored song plays");
    assert_eq!(now.path.as_deref(), Some(song.as_path()));
    // An app started without its window (a media button) does not resume.
    env.run(
        Command::Pause,
        |e| matches!(e, Event::Playback(p) if p.status == Status::Paused),
    );
    let env = env.restart_with(|c| {
        kept(c);
        c.resume_on_start = false;
    });
    std::thread::sleep(Duration::from_millis(500));
    assert_ne!(env.core.snapshot().playback.status, Status::Playing);
    let env = env.restart_with(kept);
    assert!(until(Duration::from_secs(20), || {
        env.core.snapshot().playback.status == Status::Playing
    }));
    // Without the setting it waits.
    let settings = Settings {
        resume_on_launch: false,
        ..(*env.core.snapshot().settings).clone()
    };
    env.run(Command::SetSettings { settings }, |e| {
        matches!(e, Event::Settings(_))
    });
    let env = env.restart_with(kept);
    std::thread::sleep(Duration::from_millis(500));
    assert_ne!(env.core.snapshot().playback.status, Status::Playing);
}
