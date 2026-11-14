//! Online sources end to end through the core facade: installing test
//! plugins, the `Sources` slice, unified search with the library, browsing,
//! playing source tracks streamed from a local server into a WAV file
//! (gapless, with URL refresh, lyrics and theme through the plugin),
//! collections, logins and logouts, community gating, and plugins that hang
//! or break their limits. The plugins are the test plugins
//! (`plugins/test-plugins/{echo,spin}` and `tests/plugins/catalog`); every
//! "service" is the test server here, and all audio and pictures are made up.

mod common;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::*;
use jf_core::{
    ArtworkRef, AuthFieldKind, AuthMethod, AuthStep, Collection, Command, Event, ItemKind, LyricsSource,
    PluginBadge, SearchScope, SearchUpdate, SecretStorage, Settings, SourceItem, Status, TrackRef,
};
use serde_json::json;

const PASSWORD: &str = "correct horse battery staple";

/// The base URL a request reached (for answers that point back here).
fn base(request: &Request) -> String {
    format!("http://{}", request.header("host").unwrap_or("127.0.0.1"))
}

fn hit(id: &str, title: &str, artist: &str, duration_ms: u32) -> serde_json::Value {
    json!({ "id": id, "kind": "track", "title": title, "artist": artist, "duration_ms": duration_ms })
}

/// A catalogue service: search hits, tracks with audio, albums.
#[derive(Default)]
struct Service {
    hits: Vec<serde_json::Value>,
    /// Track id → FLAC bytes.
    audio: HashMap<String, Vec<u8>>,
    /// Node id → its children, served 2 per page.
    nodes: HashMap<String, Vec<serde_json::Value>>,
    /// Track ids whose first stream request is refused (403), as an expired
    /// signed URL would be.
    refuse_first: Mutex<Vec<String>>,
    lyrics: HashMap<String, String>,
}

fn catalog_server(service: Service) -> TestServer {
    let service = Arc::new(service);
    TestServer::start(move |request| {
        let id = request.query("id").unwrap_or_default();
        match request.path() {
            "/search" => {
                // `{base}` in a hit stands for this server.
                let items = json!(service.hits).to_string().replace("{base}", &base(request));
                Response::new(200, format!(r#"{{"items": {items}}}"#))
                    .header("content-type", "application/json")
            }
            "/browse" => {
                let node = request.query("node").unwrap_or_default();
                if node.is_empty() {
                    return Response::json(json!({
                        "items": [{ "id": "albums", "kind": "folder", "title": "Albums" }]
                    }));
                }
                if node == "albums" {
                    // 250 albums in pages of `limit`.
                    let start: usize = request.query("cursor").and_then(|c| c.parse().ok()).unwrap_or(0);
                    let limit: usize = request.query("limit").and_then(|l| l.parse().ok()).unwrap_or(50);
                    let end = (start + limit).min(250);
                    let items: Vec<_> = (start..end)
                        .map(|i| json!({ "id": format!("al{i}"), "kind": "album", "title": format!("Album {i}") }))
                        .collect();
                    let next = (end < 250).then(|| end.to_string());
                    return Response::json(json!({ "items": items, "next": next }));
                }
                let Some(children) = service.nodes.get(&node) else {
                    return Response::new(404, "no such node");
                };
                let start: usize = request.query("cursor").and_then(|c| c.parse().ok()).unwrap_or(0);
                let end = (start + 2).min(children.len());
                let next = (end < children.len()).then(|| end.to_string());
                Response::json(json!({ "items": children[start..end], "next": next }))
            }
            "/resolve" if service.audio.contains_key(&id) => Response::json(json!({
                "url": format!("{}/stream/{id}.flac?sig={}", base(request), Instant::now().elapsed().as_nanos()),
                "headers": [["x-token", "abc"]],
            })),
            "/resolve" => Response::new(404, "no such track"),
            path if path.starts_with("/stream/") => {
                let id = path.trim_start_matches("/stream/").trim_end_matches(".flac");
                if request.header("x-token") != Some("abc") {
                    return Response::new(401, "no token");
                }
                {
                    let mut refuse = service.refuse_first.lock().unwrap();
                    if let Some(at) = refuse.iter().position(|r| r == id) {
                        refuse.remove(at);
                        return Response::new(403, "expired");
                    }
                }
                match service.audio.get(id) {
                    Some(bytes) => Response::ranged(request, bytes, "audio/flac"),
                    None => Response::new(404, "no audio"),
                }
            }
            "/lyrics" => match service.lyrics.get(&id) {
                Some(text) => Response::json(json!({ "format": "lrc", "text": text })),
                None => Response::new(404, "no lyrics"),
            },
            "/artwork" => Response::json(json!({
                "url": format!("{}/art/{id}.png", base(request)),
                "headers": [["x-art", "1"]],
            })),
            path if path.starts_with("/art/") => {
                if request.header("x-art") == Some("1") {
                    Response::new(200, png([200, 30, 40], 32)).header("content-type", "image/png")
                } else {
                    Response::new(401, "no art header")
                }
            }
            _ => Response::new(404, "not found"),
        }
    })
}

fn searched(env: &Env, query: &str, scope: SearchScope) -> Vec<Arc<SearchUpdate>> {
    let id = env.core.send(Command::Search {
        query: query.into(),
        scope,
    });
    let events = wait_for(
        &env.events,
        Duration::from_secs(30),
        |e| matches!(e, Event::SearchResults(u) if u.request == id && u.done),
    );
    events
        .into_iter()
        .filter_map(|e| match e {
            Event::SearchResults(u) if u.request == id => Some(u),
            _ => None,
        })
        .collect()
}

fn track_of(item: &SourceItem) -> TrackRef {
    item.track
        .clone()
        .unwrap_or_else(|| panic!("{} is not playable", item.title))
}

fn find<'a>(items: &'a [SourceItem], title: &str) -> &'a SourceItem {
    items
        .iter()
        .find(|i| i.title == title)
        .unwrap_or_else(|| panic!("no {title:?} in {items:#?}"))
}

/// Plays `tracks` to the end of the queue and returns the events.
fn play_to_end(env: &Env, tracks: Vec<TrackRef>) -> Vec<Event> {
    env.run(
        Command::PlayTracks { tracks, start: 0 },
        |e| matches!(e, Event::Playback(p) if p.status == Status::Ended),
    )
}

fn tag_wav(path: &Path, title: &str, artist: &str, isrc: Option<&str>) {
    use lofty::config::WriteOptions;
    use lofty::prelude::*;
    use lofty::tag::{Tag, TagType};
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_title(title.into());
    tag.set_artist(artist.into());
    tag.set_album("Test Album".into());
    if let Some(isrc) = isrc {
        tag.insert_text(ItemKey::Isrc, isrc.into());
    }
    tag.save_to_path(path, WriteOptions::default()).unwrap();
}

#[test]
fn installs_plugins_and_publishes_the_sources_slice() {
    let env = Env::new();
    let initial = env.sources();
    assert!(initial.plugins.is_empty());
    assert!(initial.unavailable.is_none());
    assert!(!initial.allow_community);

    let icon = png([10, 120, 200], 16);
    let echo = env.install("echo", &echo_manifest(), ECHO, true, Some(icon.clone()));
    assert_eq!(echo.badge, PluginBadge::Official);
    assert!(echo.enabled && !echo.needs_disclosure);
    assert_eq!(echo.signing_key, None);
    assert_eq!(echo.description.as_deref(), Some("A plugin for tests"));
    assert!(echo.capabilities.iter().any(|c| c == "resolve_stream"));
    assert_eq!(
        echo.auth_methods,
        vec![
            AuthMethod::Qr,
            AuthMethod::Oauth,
            AuthMethod::PasswordExchange,
            AuthMethod::ApiKey
        ]
    );
    // As the package wrote it, so forms list the settings in the plugin's order.
    assert_eq!(echo.settings_schema_json, common::SCHEMA);
    assert!(!echo.vendor_playback);
    let icon_path = echo.icon.clone().expect("an icon file");
    assert_eq!(std::fs::read(&icon_path).unwrap(), icon, "the package's icon.png");

    // Settings are validated by the plugin's schema and shown.
    env.set_server(ECHO, "http://127.0.0.1:9");
    let error = env.fails(Command::SetPluginSettings {
        id: ECHO.into(),
        settings_json: r#"{"server_url": 5}"#.into(),
    });
    assert!(error.contains("invalid settings"), "{error}");

    // Off and on again.
    env.run(
        Command::SetPluginEnabled {
            id: ECHO.into(),
            enabled: false,
        },
        |e| plugin_in(e, ECHO).is_some_and(|p| !p.enabled),
    );
    env.run(
        Command::SetPluginEnabled {
            id: ECHO.into(),
            enabled: true,
        },
        |e| plugin_in(e, ECHO).is_some_and(|p| p.enabled),
    );

    // Installed plugins are there after a restart.
    let env = env.restart();
    let echo = env.summary(ECHO);
    assert!(echo.enabled);
    assert!(echo.settings_json.contains("127.0.0.1:9"));
    assert_eq!(env.sources().secret_storage, SecretStorage::Keystore);

    env.run(
        Command::UninstallPlugin { id: ECHO.into() },
        |e| matches!(e, Event::Sources(s) if s.plugins.is_empty()),
    );
    assert!(!icon_path.exists(), "the cached icon goes with the plugin");
    let error = env.fails(Command::UninstallPlugin { id: ECHO.into() });
    assert!(error.contains("no source plugin"), "{error}");
}

#[test]
fn unified_search_merges_the_library_with_the_sources() {
    let env = Env::new();
    let music = env.dir.path().join("music");
    std::fs::create_dir_all(&music).unwrap();
    sine_wav(&music.join("blue.wav"), 440.0, 1.0);
    tag_wav(&music.join("blue.wav"), "Blue Hour", "Test Artist", None);
    sine_wav(&music.join("isrc.wav"), 550.0, 1.0);
    tag_wav(
        &music.join("isrc.wav"),
        "Bluebird",
        "Test Artist",
        Some("USRC17607839"),
    );
    env.run(Command::AddRoot { path: music }, |e| {
        matches!(e, Event::Scan(jf_core::ScanStatus::Finished(_)))
    });

    let server = catalog_server(Service {
        hits: vec![
            // Same title (once normalised), primary artist and duration.
            hit("c1", "Blue Hour (Remastered)", "Test Artist feat. Guest", 1_400),
            // Same ISRC under another name.
            json!({ "id": "c2", "kind": "track", "title": "Bluebird - Live", "artist": "Someone Else",
                    "duration_ms": 200_000, "isrc": "us-rc1-76-07839" }),
            hit("c3", "Blue Only Online", "Test Artist", 3_000),
            json!({ "id": "c4", "kind": "track", "title": "Blue Locked", "artist": "Test Artist",
                    "playable": false, "reason": "region" }),
            json!({ "id": "al1", "kind": "album", "title": "Blue Album", "artist": "Test Artist" }),
        ],
        ..Service::default()
    });
    env.plugin("catalog", &catalog_manifest(), CATALOG, &server.url());

    let updates = searched(&env, "blue", SearchScope::All);
    let order: Vec<(&str, bool)> = updates.iter().map(|u| (u.source.as_str(), u.done)).collect();
    assert_eq!(
        order,
        vec![("library", false), ("library", false), (CATALOG, true)],
        "the library at once; when the plugin answers, the library again (two of its tracks are \
         online too), then the plugin's own section"
    );
    let first = &updates[0];
    assert!(
        first
            .items
            .iter()
            .all(|i| i.source == "library" && i.also_in.is_empty())
    );
    let library = &updates[1];
    assert!(library.items.iter().all(|i| i.source == "library"));
    assert_eq!(library.error, None);
    let catalog = &updates[2];
    assert_eq!(catalog.error, None);
    assert!(catalog.items.iter().all(|i| i.source == CATALOG));

    let blue = find(&library.items, "Blue Hour");
    assert!(matches!(blue.track, Some(TrackRef::Library(_))));
    assert_eq!(blue.also_in, vec![CATALOG.to_string()]);
    let bluebird = find(&library.items, "Bluebird");
    assert!(
        matches!(bluebird.track, Some(TrackRef::Library(_))),
        "matched by ISRC"
    );
    assert_eq!(bluebird.also_in, vec![CATALOG.to_string()]);
    assert_eq!(
        first.items.len(),
        library.items.len(),
        "the same rows, now with also_in"
    );

    // The plugin's section holds what the library does not have.
    let titles: Vec<&str> = catalog.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["Blue Only Online", "Blue Locked", "Blue Album"],
        "in the plugin's order"
    );
    let online = find(&catalog.items, "Blue Only Online");
    let TrackRef::Source(t) = track_of(online) else {
        panic!("a source track")
    };
    assert_eq!(
        (t.plugin.as_str(), t.id.as_str(), t.duration_ms),
        (CATALOG, "c3", Some(3_000))
    );
    let locked = find(&catalog.items, "Blue Locked");
    assert!(!locked.playable && locked.track.is_none());
    assert_eq!(
        locked.unplayable_reason.as_deref(),
        Some("Not available in your region")
    );
    assert_eq!(find(&catalog.items, "Blue Album").kind, ItemKind::Album);

    // One source alone.
    let only = searched(&env, "blue", SearchScope::Plugin { id: CATALOG.into() });
    assert_eq!(only.len(), 1);
    assert!(only[0].done && only[0].items.iter().all(|i| i.source == CATALOG));
    assert_eq!(only[0].items.len(), 5);

    // The library alone does not ask the plugin.
    let asked = server.count("/search");
    let library_only = searched(&env, "blue", SearchScope::Library);
    assert_eq!(library_only.len(), 1);
    assert!(library_only[0].done);
    assert_eq!(server.count("/search"), asked);

    // A newer search supersedes an older one that has not reached the
    // sources yet: the older gets its library answer only.
    let first = env.core.send(Command::Search {
        query: "blu".into(),
        scope: SearchScope::All,
    });
    let second = env.core.send(Command::Search {
        query: "blue".into(),
        scope: SearchScope::All,
    });
    let events = wait_for(
        &env.events,
        Duration::from_secs(30),
        |e| matches!(e, Event::SearchResults(u) if u.request == second && u.done),
    );
    let late: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, Event::SearchResults(u) if u.request == first && u.source == CATALOG))
        .collect();
    assert!(late.is_empty(), "{late:?}");
    assert_eq!(
        server.count("/search"),
        asked + 1,
        "only the newer search reached the plugin"
    );

    let error = env.fails(Command::Search {
        query: "x".into(),
        scope: SearchScope::Plugin {
            id: "org.example.missing".into(),
        },
    });
    assert!(error.contains("no source plugin"), "{error}");
}

#[test]
fn browses_a_catalogue_page_by_page() {
    let env = Env::new();
    let server = catalog_server(Service {
        nodes: HashMap::from([(
            "al3".to_string(),
            vec![hit("t1", "One", "A", 1_000), hit("t2", "Two", "A", 1_000)],
        )]),
        ..Service::default()
    });
    env.plugin("catalog", &catalog_manifest(), CATALOG, &server.url());

    let browse = |node: Option<&str>, page: u32| {
        let id = env.core.send(Command::Browse {
            plugin: CATALOG.into(),
            node: node.map(str::to_owned),
            page,
        });
        let events = wait_for(
            &env.events,
            Duration::from_secs(30),
            |e| matches!(e, Event::BrowsePage(p) if p.request == id),
        );
        match events.last() {
            Some(Event::BrowsePage(p)) => p.clone(),
            _ => unreachable!(),
        }
    };
    let root = browse(None, 0);
    assert_eq!(root.error, None);
    assert_eq!(root.items.len(), 1);
    assert_eq!(
        (root.items[0].id.as_str(), root.items[0].kind),
        ("albums", ItemKind::Folder)
    );
    assert_eq!(root.next_page, None);

    let first = browse(Some("albums"), 0);
    assert_eq!(first.items.len(), 100);
    assert_eq!(first.items[0].id, "al0");
    assert_eq!(first.next_page, Some(1));
    assert_eq!(first.node.as_deref(), Some("albums"));
    // The plugin got the node back whole: a folder, as it listed it.
    let asked = server.with_path("/browse");
    assert_eq!(asked.last().unwrap().query("kind").as_deref(), Some("folder"));

    // Page 2 straight away: the core fetches page 1 for its cursor first.
    let third = browse(Some("albums"), 2);
    assert_eq!(third.items.len(), 50);
    assert_eq!(third.items[0].id, "al200");
    assert_eq!(third.next_page, None);
    assert_eq!(server.count("/browse"), asked.len() + 2);

    // An album listed on page 0 opens as an album.
    let album = browse(Some("al3"), 0);
    assert_eq!(
        server
            .with_path("/browse")
            .last()
            .unwrap()
            .query("kind")
            .as_deref(),
        Some("album")
    );
    assert_eq!(album.items.len(), 2);
    assert!(album.items.iter().all(|i| i.track.is_some()));

    let missing = browse(Some("nothing"), 0);
    assert!(missing.error.is_some() && missing.items.is_empty());
}

#[test]
fn plays_a_source_track_with_lyrics_and_theme_from_its_plugin() {
    let env = Env::new();
    let server = catalog_server(Service {
        hits: vec![
            json!({ "id": "t1", "kind": "track", "title": "Tone One", "artist": "Test Artist",
                           "album": "Tones", "duration_ms": 1_500, "artwork": "{base}/listed.png" }),
        ],
        audio: HashMap::from([("t1".to_string(), sine_flac(440.0, 1.5))]),
        lyrics: HashMap::from([("t1".to_string(), "[00:00.50]A test line\n".to_string())]),
        ..Service::default()
    });
    env.plugin("catalog", &catalog_manifest(), CATALOG, &server.url());
    let results = searched(&env, "tone", SearchScope::Plugin { id: CATALOG.into() });
    let track = track_of(find(&results[0].items, "Tone One"));

    let events = play_to_end(&env, vec![track.clone()]);
    let now: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::NowPlaying(Some(n)) => Some(n.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(now.len(), 1, "{:?}", events.iter().map(short).collect::<Vec<_>>());
    let np = &now[0];
    assert_eq!(np.track, track);
    assert_eq!(
        (np.title.as_str(), np.artist.as_deref(), np.album.as_deref()),
        ("Tone One", Some("Test Artist"), Some("Tones"))
    );
    assert_eq!(np.duration_ms, Some(1_500));
    assert_eq!(np.path, None);
    assert_eq!(
        np.artwork,
        Some(ArtworkRef::Url(format!("{}/listed.png", server.url())))
    );
    let format = np.format.as_ref().expect("the stream's format");
    assert_eq!((format.codec.as_str(), format.sample_rate), ("flac", RATE));

    // Buffering while the URL was resolved, then playing.
    let buffering: Vec<bool> = events
        .iter()
        .filter_map(|e| match e {
            Event::Buffering { uid, buffering } if *uid == np.uid => Some(*buffering),
            _ => None,
        })
        .collect();
    assert_eq!(buffering.first(), Some(&true));
    assert_eq!(buffering.last(), Some(&false));

    // The audio came through the plugin's stream URL, with its headers.
    let (rate, left) = read_wav(&env.out);
    let seconds = left.len() as f64 / f64::from(rate);
    assert!((seconds - 1.5).abs() < 0.05, "rendered {seconds} s");
    let f = frequency(&left[2_000..left.len() - 2_000], rate);
    assert!((f - 440.0).abs() < 5.0, "{f} Hz");
    let streams = server.with_path("/stream/t1.flac");
    assert!(!streams.is_empty());
    assert!(streams.iter().all(|r| r.header("x-token") == Some("abc")));
    assert_eq!(server.count("/resolve"), 1);

    // Lyrics and theme colours came through the plugin too (the media
    // worker may finish after the queue has ended).
    let mut lyrics = None;
    let mut theme = None;
    for e in events.iter() {
        match e {
            Event::Lyrics(l) if l.uid == np.uid => lyrics = Some(l.clone()),
            Event::Theme(t) if t.uid == Some(np.uid) => theme = Some(t.clone()),
            _ => {}
        }
    }
    if lyrics.is_none() || theme.is_none() {
        wait_for(&env.events, Duration::from_secs(20), |e| {
            match e {
                Event::Lyrics(l) if l.uid == np.uid => lyrics = Some(l.clone()),
                Event::Theme(t) if t.uid == Some(np.uid) => theme = Some(t.clone()),
                _ => {}
            }
            lyrics.is_some() && theme.is_some()
        });
    }
    let lyrics = lyrics.unwrap();
    assert_eq!(lyrics.source, Some(LyricsSource::Plugin(CATALOG.into())));
    let doc = lyrics.doc.as_ref().expect("lyrics");
    assert_eq!(doc.lines.len(), 1);
    assert_eq!(
        doc.source.origin.as_deref(),
        Some("Test org.jellyfesh.test.catalog")
    );
    assert!(lyrics.timeline.is_some(), "synced lyrics have a timeline");
    let theme = theme.unwrap();
    assert!(theme.from_artwork, "the red artwork gives the hue");
    assert!(theme.hue < 45.0 || theme.hue > 330.0, "red, got {}", theme.hue);
    assert!(theme.backdrop_light.as_ref().is_some_and(|p| p.is_file()));
    let art = server.with_path("/art/t1.png");
    assert_eq!(art.len(), 1);
    assert_eq!(art[0].header("x-art"), Some("1"), "the plugin's artwork headers");
}

#[test]
fn source_tracks_join_gaplessly_and_refused_urls_are_resolved_again() {
    // Paced like a device, so the next track is prepared while one plays.
    let env = Env::with(|c| {
        if let jf_core::OutputKind::WavFile { realtime, .. } = &mut c.output {
            *realtime = true;
        }
    });
    let server = catalog_server(Service {
        hits: vec![hit("t1", "First", "A", 1_000), hit("t2", "Second", "A", 1_000)],
        audio: HashMap::from([
            ("t1".to_string(), sine_flac(440.0, 1.0)),
            ("t2".to_string(), sine_flac(660.0, 1.0)),
        ]),
        // The second track's first URL has expired by the time it is fetched.
        refuse_first: Mutex::new(vec!["t2".to_string()]),
        ..Service::default()
    });
    env.plugin("catalog", &catalog_manifest(), CATALOG, &server.url());
    let results = searched(&env, "x", SearchScope::Plugin { id: CATALOG.into() });
    let tracks = vec![
        track_of(find(&results[0].items, "First")),
        track_of(find(&results[0].items, "Second")),
    ];

    let events = play_to_end(&env, tracks);
    let titles: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::NowPlaying(Some(n)) => Some(n.title.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(titles, vec!["First", "Second"]);
    assert!(
        !events.iter().any(|e| matches!(e, Event::Error { .. })),
        "{:?}",
        events.iter().map(short).collect::<Vec<_>>()
    );

    // The refusal was answered by resolving the URL again through the
    // plugin (the engine's resolver), and the stream went on.
    let resolves: Vec<_> = server
        .with_path("/resolve")
        .iter()
        .filter_map(|r| r.query("id"))
        .collect();
    assert_eq!(
        resolves.iter().filter(|id| *id == "t2").count(),
        2,
        "{resolves:?}"
    );
    let t2: Vec<_> = server.with_path("/stream/t2.flac");
    assert!(t2.len() >= 2);
    // The second track was resolved and fetched while the first played, so
    // the engine joined them gaplessly.
    let t1_start = server.with_path("/stream/t1.flac")[0].at;
    let ahead = t2[0].at.duration_since(t1_start);
    assert!(
        ahead < Duration::from_millis(900),
        "the second stream opened {ahead:?} after the first"
    );

    // Both tracks back to back with no gap: 2 s of tone, the second at 660 Hz.
    let (rate, left) = read_wav(&env.out);
    let seconds = left.len() as f64 / f64::from(rate);
    assert!((seconds - 2.0).abs() < 0.05, "rendered {seconds} s");
    let quiet = left[1_000..left.len() - 1_000]
        .windows(64)
        .filter(|w| w.iter().all(|s| s.abs() < 1e-4))
        .count();
    assert_eq!(quiet, 0, "no silence between the tracks");
    let second = &left[left.len() / 2 + 2_000..left.len() - 2_000];
    let f = frequency(second, rate);
    assert!((f - 660.0).abs() < 8.0, "{f} Hz");
}

#[test]
fn plays_and_queues_whole_source_collections() {
    let env = Env::new();
    let locked = |id: &str| json!({ "id": id, "kind": "track", "title": format!("Locked {id}"), "playable": false, "reason": "paywall" });
    let server = catalog_server(Service {
        hits: vec![
            json!({ "id": "al1", "kind": "album", "title": "Album One", "artist": "A" }),
            json!({ "id": "al2", "kind": "album", "title": "All Locked", "artist": "A" }),
        ],
        audio: HashMap::from([
            ("t1".to_string(), sine_flac(440.0, 0.4)),
            ("t3".to_string(), sine_flac(550.0, 0.4)),
            ("t4".to_string(), sine_flac(660.0, 0.4)),
        ]),
        // Three pages of two: every page is fetched.
        nodes: HashMap::from([
            (
                "al1".to_string(),
                vec![
                    hit("t1", "One", "A", 400),
                    locked("t2"),
                    hit("t3", "Three", "A", 400),
                    json!({ "id": "sub", "kind": "folder", "title": "Extras" }),
                    hit("t4", "Four", "A", 400),
                ],
            ),
            ("al2".to_string(), vec![locked("t9")]),
        ]),
        ..Service::default()
    });
    env.plugin("catalog", &catalog_manifest(), CATALOG, &server.url());
    searched(&env, "album", SearchScope::Plugin { id: CATALOG.into() });

    // Start at the locked track: it is left out, and play starts at the next.
    let events = env.run(
        Command::PlayCollection {
            collection: Collection::Source {
                plugin: CATALOG.into(),
                id: "al1".into(),
            },
            start: 1,
            shuffle: false,
        },
        |e| matches!(e, Event::NowPlaying(Some(n)) if n.title == "Three"),
    );
    assert!(events.iter().any(|e| matches!(e, Event::Queue(q) if q.len == 3)));
    let queue = env.core.queue_window(0, 10);
    let ids: Vec<_> = queue
        .iter()
        .map(|i| match &i.track {
            TrackRef::Source(t) => t.id.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(ids, vec!["t1", "t3", "t4"]);
    assert!(
        server
            .with_path("/browse")
            .iter()
            .all(|r| r.query("kind").as_deref() == Some("album"))
    );
    assert_eq!(server.count("/browse"), 3, "three pages");

    env.run(
        Command::EnqueueCollection {
            collection: Collection::Source {
                plugin: CATALOG.into(),
                id: "al1".into(),
            },
            play_next: false,
        },
        |e| matches!(e, Event::Queue(q) if q.len == 6),
    );

    let error = env.fails(Command::PlayCollection {
        collection: Collection::Source {
            plugin: CATALOG.into(),
            id: "al2".into(),
        },
        start: 0,
        shuffle: false,
    });
    assert_eq!(error, "nothing to play: Needs a subscription or a purchase");
}

fn echo_server() -> TestServer {
    TestServer::start(|request| match request.path() {
        "/login"
            if request.form("username").as_deref() == Some("alice")
                && request.form("password").as_deref() == Some(PASSWORD) =>
        {
            Response::json(json!({ "token": "tok-alice" }))
        }
        "/login" => Response::new(401, "no"),
        "/logout" => Response::new(200, "bye"),
        "/search" => Response::json(json!([{ "id": "e1", "title": "Echo hit" }])),
        _ => Response::new(404, "not found"),
    })
}

fn auth_steps(env: &Env, plugin: &str, until: impl Fn(&AuthStep) -> bool) -> Vec<(u64, AuthStep)> {
    let events = wait_for(
        &env.events,
        Duration::from_secs(30),
        |e| matches!(e, Event::Auth(a) if a.plugin == plugin && until(&a.step)),
    );
    events
        .into_iter()
        .filter_map(|e| match e {
            Event::Auth(a) if a.plugin == plugin => Some((a.flow, a.step)),
            _ => None,
        })
        .collect()
}

#[test]
fn signs_in_and_out_with_auth_events_and_never_keeps_a_password() {
    let env = Env::new();
    let server = echo_server();
    env.plugin("echo", &echo_manifest(), ECHO, &server.url());

    // A QR login: show the code, wait for the phone, done.
    env.core.send(Command::BeginLogin {
        plugin: ECHO.into(),
        method: AuthMethod::Qr,
    });
    let steps = auth_steps(&env, ECHO, |s| {
        matches!(s, AuthStep::Done { .. } | AuthStep::Failed { .. })
    });
    let flow = steps[0].0;
    assert!(steps.iter().all(|(f, _)| *f == flow));
    match &steps[0].1 {
        AuthStep::ShowQr {
            data,
            hint,
            expires_at_ms,
        } => {
            assert!(data.starts_with("jf-test://login/"), "{data}");
            assert_eq!(hint.as_deref(), Some("Scan with the test app"));
            assert!(expires_at_ms.is_some());
        }
        other => panic!("{other:?}"),
    }
    assert!(
        steps
            .iter()
            .any(|(_, s)| matches!(s, AuthStep::Waiting { message } if message == "Confirm on your phone"))
    );
    assert_eq!(
        steps.last().unwrap().1,
        AuthStep::Done {
            account: "QR User".into()
        }
    );
    wait_for(&env.events, Duration::from_secs(10), |e| {
        plugin_in(e, ECHO).is_some_and(|p| p.account.as_deref() == Some("QR User"))
    });
    let token_key = format!("jellyfesh/{ECHO}/token");
    assert!(
        env.secrets.keys().contains(&token_key),
        "{:?}",
        env.secrets.keys()
    );

    // Logout deletes the secrets and the account; the plugin may revoke.
    env.run(Command::Logout { plugin: ECHO.into() }, |e| {
        plugin_in(e, ECHO).is_some_and(|p| p.account.is_none())
    });
    assert!(
        !env.secrets
            .keys()
            .iter()
            .any(|k| k.starts_with(&format!("jellyfesh/{ECHO}/"))),
        "{:?}",
        env.secrets.keys()
    );
    assert_eq!(server.count("/logout"), 1);

    // A password login: the form, the answer, a token, never the password.
    env.core.send(Command::BeginLogin {
        plugin: ECHO.into(),
        method: AuthMethod::PasswordExchange,
    });
    let steps = auth_steps(&env, ECHO, |s| matches!(s, AuthStep::AskFields { .. }));
    let (flow, AuthStep::AskFields { fields }) = steps.last().unwrap().clone() else {
        unreachable!()
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(
        (fields[0].name.as_str(), fields[0].kind, fields[0].secret),
        ("username", AuthFieldKind::Text, false)
    );
    assert_eq!((fields[1].name.as_str(), fields[1].secret), ("password", true));
    assert!(fields.iter().all(|f| f.required));
    env.core.send(Command::SubmitLogin {
        plugin: ECHO.into(),
        flow,
        fields: vec![
            ("username".into(), "alice".into()),
            ("password".into(), PASSWORD.into()),
        ],
    });
    let steps = auth_steps(&env, ECHO, |s| {
        matches!(s, AuthStep::Done { .. } | AuthStep::Failed { .. })
    });
    assert_eq!(
        steps.last().unwrap().1,
        AuthStep::Done {
            account: "alice".into()
        }
    );
    assert!(
        !env.secrets.any_value_contains(PASSWORD),
        "the password is never stored"
    );
    assert!(
        !files_contain(env.dir.path(), PASSWORD),
        "nor written anywhere in the app's folders"
    );

    // A login can be cancelled; unknown flows are refused.
    env.core.send(Command::BeginLogin {
        plugin: ECHO.into(),
        method: AuthMethod::ApiKey,
    });
    let steps = auth_steps(&env, ECHO, |s| matches!(s, AuthStep::AskFields { .. }));
    let flow = steps.last().unwrap().0;
    env.core.send(Command::CancelLogin {
        plugin: ECHO.into(),
        flow,
    });
    let steps = auth_steps(&env, ECHO, |s| matches!(s, AuthStep::Failed { .. }));
    assert_eq!(
        steps.last().unwrap().1,
        AuthStep::Failed {
            message: "cancelled".into()
        }
    );
    let error = env.fails(Command::SubmitLogin {
        plugin: ECHO.into(),
        flow: 999,
        fields: Vec::new(),
    });
    assert!(error.contains("no login 999"), "{error}");
    let error = env.fails(Command::BeginLogin {
        plugin: ECHO.into(),
        method: AuthMethod::CookieImport,
    });
    assert!(error.contains("cookie-import"), "{error}");
}

fn files_contain(dir: &Path, needle: &str) -> bool {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            for entry in std::fs::read_dir(&path).unwrap() {
                stack.push(entry.unwrap().path());
            }
        } else if let Ok(bytes) = std::fs::read(&path)
            && bytes.windows(needle.len()).any(|w| w == needle.as_bytes())
        {
            return true;
        }
    }
    false
}

#[test]
fn community_plugins_wait_for_the_disclosure() {
    let env = Env::new();
    let server = echo_server();
    let path = package(&env.incoming(), "echo", &echo_manifest(), false, None);

    // Off by default: refused, naming the package.
    let error = env.fails(Command::InstallPlugin { path: path.clone() });
    assert!(error.contains("community plugins are turned off"), "{error}");
    assert!(error.contains(ECHO), "{error}");

    // Allowed: installed off, with the disclosure still to accept.
    let settings = Settings {
        allow_community_plugins: true,
        ..(*env.core.snapshot().settings).clone()
    };
    env.run(
        Command::SetSettings { settings },
        |e| matches!(e, Event::Sources(s) if s.allow_community),
    );
    let echo = {
        let events = env.run(Command::InstallPlugin { path: path.clone() }, |e| {
            has_plugin(e, ECHO)
        });
        plugin_in(events.last().unwrap(), ECHO).unwrap()
    };
    assert_eq!(echo.badge, PluginBadge::Community);
    assert!(!echo.enabled && echo.needs_disclosure);
    let error = env.fails(Command::SetPluginEnabled {
        id: ECHO.into(),
        enabled: true,
    });
    assert!(error.contains("AcknowledgeUnofficial"), "{error}");
    env.set_server(ECHO, &server.url());
    assert_eq!(
        searched(&env, "x", SearchScope::All).len(),
        1,
        "a plugin that is off is not searched"
    );

    // Accepting the disclosure turns it on.
    env.run(Command::AcknowledgeUnofficial { id: ECHO.into() }, |e| {
        plugin_in(e, ECHO).is_some_and(|p| p.enabled && !p.needs_disclosure)
    });
    let updates = searched(&env, "x", SearchScope::All);
    assert_eq!(updates.last().unwrap().source, ECHO);

    // Community plugins off again: it stays installed but cannot be used.
    let settings = Settings {
        allow_community_plugins: false,
        ..(*env.core.snapshot().settings).clone()
    };
    env.run(Command::SetSettings { settings }, |e| {
        plugin_in(e, ECHO).is_some_and(|p| !p.enabled)
    });
    let echo = env.summary(ECHO);
    assert_eq!(
        echo.disabled_reason.as_deref(),
        Some("Community plugins are turned off")
    );
    assert_eq!(searched(&env, "x", SearchScope::All).len(), 1);
    let error = env.fails(Command::Search {
        query: "x".into(),
        scope: SearchScope::Plugin { id: ECHO.into() },
    });
    assert!(error.contains("community plugins are turned off"), "{error}");

    // The consent is kept, and accepting again turns community plugins on.
    let env = env.restart();
    assert!(!env.summary(ECHO).needs_disclosure);
    env.run(Command::AcknowledgeUnofficial { id: ECHO.into() }, |e| {
        matches!(e, Event::Sources(s) if s.allow_community && s.plugins.iter().any(|p| p.id == ECHO && p.enabled))
    });
    assert!(env.core.snapshot().settings.allow_community_plugins);

    // A disclosure accepted before the install turns the plugin on with it.
    env.run(
        Command::UninstallPlugin { id: ECHO.into() },
        |e| matches!(e, Event::Sources(s) if s.plugins.is_empty()),
    );
    env.run(Command::AcknowledgeUnofficial { id: ECHO.into() }, |e| {
        matches!(e, Event::Sources(_))
    });
    let events = env.run(Command::InstallPlugin { path }, |e| has_plugin(e, ECHO));
    let echo = plugin_in(events.last().unwrap(), ECHO).unwrap();
    assert!(echo.enabled && !echo.needs_disclosure);
}

#[test]
fn a_plugin_that_hangs_never_blocks_the_actor() {
    // The spinning plugin may burn 5 s of CPU per call; searches wait 600 ms
    // for each source.
    let env = Env::with(|c| {
        c.sources.host.limits.cpu = Duration::from_secs(5);
        c.sources.host.limits.wall = Duration::from_secs(6);
        c.sources.search_timeout = Duration::from_millis(600);
    });
    let server = catalog_server(Service {
        hits: vec![hit("c1", "Quick", "A", 1_000)],
        ..Service::default()
    });
    env.plugin("catalog", &catalog_manifest(), CATALOG, &server.url());
    env.plugin("spin", &spin_manifest(), SPIN, &server.url());

    let started = Instant::now();
    let search = env.core.send(Command::Search {
        query: "q".into(),
        scope: SearchScope::All,
    });
    // While the spinning plugin hogs a worker, the actor answers at once.
    std::thread::sleep(Duration::from_millis(150));
    let asked = Instant::now();
    let mut events = env.run(
        Command::SetVolume { volume: 0.5 },
        |e| matches!(e, Event::Playback(p) if (p.volume - 0.5).abs() < 1e-6),
    );
    assert!(
        asked.elapsed() < Duration::from_millis(500),
        "{:?}",
        asked.elapsed()
    );
    let snapshot_at = Instant::now();
    let _ = env.core.snapshot();
    assert!(snapshot_at.elapsed() < Duration::from_millis(100));

    if !events
        .iter()
        .any(|e| matches!(e, Event::SearchResults(u) if u.request == search && u.done))
    {
        events.extend(wait_for(
            &env.events,
            Duration::from_secs(30),
            |e| matches!(e, Event::SearchResults(u) if u.request == search && u.done),
        ));
    }
    let updates: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::SearchResults(u) if u.request == search => Some(u.clone()),
            _ => None,
        })
        .collect();
    let sources: Vec<_> = updates.iter().map(|u| u.source.as_str()).collect();
    assert_eq!(sources, vec!["library", CATALOG, SPIN], "the quick source first");
    assert!(updates[1].items.iter().any(|i| i.title == "Quick"));
    let late = updates.last().unwrap();
    assert!(
        late.error
            .as_deref()
            .is_some_and(|e| e.contains("did not answer in time")),
        "{late:?}"
    );
    assert!(late.items.is_empty() && late.done);
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "the search ended at its deadline, not the plugin's: {:?}",
        started.elapsed()
    );
}

#[test]
fn a_plugin_the_host_turns_off_shows_in_the_sources_slice() {
    let env = Env::new();
    let server = catalog_server(Service::default());
    env.plugin("spin", &spin_manifest(), SPIN, &server.url());
    // Each search breaks the CPU limit; the third in a row turns it off.
    for n in 1..=3 {
        let updates = searched(&env, "q", SearchScope::Plugin { id: SPIN.into() });
        let error = updates.last().unwrap().error.clone().unwrap_or_default();
        assert!(error.contains("CPU"), "search {n}: {error}");
    }
    // The slice may have gone out while the search was waited for: the
    // snapshot has it then, an event otherwise.
    if env.summary(SPIN).enabled {
        wait_for(&env.events, Duration::from_secs(10), |e| {
            plugin_in(e, SPIN).is_some_and(|p| !p.enabled)
        });
    }
    let spin = env.summary(SPIN);
    assert!(!spin.enabled);
    let reason = spin.disabled_reason.expect("a reason");
    assert!(reason.contains("3 times"), "{reason}");
    let error = env.fails(Command::Search {
        query: "q".into(),
        scope: SearchScope::Plugin { id: SPIN.into() },
    });
    assert!(error.contains("turned off"), "{error}");
}

#[test]
fn bundled_plugins_are_installed_updated_and_stay_uninstalled() {
    // The packages an app ships in its bundle. The session is kept, as in
    // the apps, so the settings store remembers what the person uninstalled.
    let bundle = tempfile::tempdir().unwrap();
    let dir = bundle.path();
    let catalog_v1 = package_as(dir, "catalog-1.0.0.jfpkg", "catalog", &catalog_manifest(), true);
    let catalog_v2 = package_as(
        dir,
        "catalog-1.1.0.jfpkg",
        "catalog",
        &with_version(&catalog_manifest(), "1.1.0"),
        true,
    );
    let echo = package_as(dir, "echo-1.0.0.jfpkg", "echo", &echo_manifest(), true);
    let unsigned = package_as(dir, "spin-unsigned.jfpkg", "spin", &spin_manifest(), false);
    // A fresh plugin last in the list shows that the core has gone through
    // the ones before it.
    let marker = |n: u32| {
        let id = format!("org.jellyfesh.test.marker{n}");
        let manifest = manifest(
            &id,
            &["search", "browse", "resolve_stream", "lyrics", "artwork"],
            &[],
        );
        (
            id,
            package_as(dir, &format!("marker{n}.jfpkg"), "catalog", &manifest, true),
        )
    };
    let bundled = |paths: Vec<std::path::PathBuf>| {
        move |c: &mut jf_core::Config| {
            c.persist_session = true;
            c.restore_session = true;
            c.bundled_plugins = paths;
        }
    };
    let started = |env: &Env, id: &str| {
        // Subscribed before this check: an install after it is an event.
        if !env.sources().plugins.iter().any(|p| p.id == id) {
            wait_for(&env.events, Duration::from_secs(60), |e| has_plugin(e, id));
        }
    };
    let package_file = |env: &Env| {
        env.dir
            .path()
            .join("data/plugins/packages")
            .join(format!("{CATALOG}.jfpkg"))
    };
    let modified = |path: &Path| std::fs::metadata(path).unwrap().modified().unwrap();

    // First start: the missing packages are installed, checked like any
    // package: an unsigned one is a community plugin, refused while those
    // are off.
    let env = Env::with(bundled(vec![catalog_v1.clone(), unsigned, echo.clone()]));
    let events = wait_for(
        &env.events,
        Duration::from_secs(60),
        |e| matches!(e, Event::Sources(s) if s.plugins.len() == 2),
    );
    let refused: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Error { message } => Some(message.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(
        refused[0].contains("spin-unsigned.jfpkg") && refused[0].contains("community plugins are turned off"),
        "{}",
        refused[0]
    );
    let catalog = env.summary(CATALOG);
    assert_eq!(
        (catalog.version.as_str(), catalog.badge, catalog.enabled),
        ("1.0.0", PluginBadge::Official, true)
    );
    assert!(env.summary(ECHO).enabled);
    let installed_at = modified(&package_file(&env));

    // The same packages again: nothing is reinstalled.
    let (first, path) = marker(1);
    let env = env.restart_with(bundled(vec![catalog_v1.clone(), echo.clone(), path]));
    started(&env, &first);
    assert_eq!(modified(&package_file(&env)), installed_at);

    // A newer bundled version updates the plugin; an older one does not.
    env.set_server(CATALOG, "http://127.0.0.1:9");
    let (second, path) = marker(2);
    let env = env.restart_with(bundled(vec![catalog_v2.clone(), path]));
    started(&env, &second);
    let catalog = env.summary(CATALOG);
    assert_eq!(catalog.version, "1.1.0");
    assert!(
        catalog.settings_json.contains("127.0.0.1:9"),
        "an update keeps the settings"
    );
    let (third, path) = marker(3);
    let env = env.restart_with(bundled(vec![catalog_v1.clone(), path]));
    started(&env, &third);
    assert_eq!(env.summary(CATALOG).version, "1.1.0");

    // A bundled plugin the person uninstalls stays uninstalled.
    env.run(
        Command::UninstallPlugin { id: CATALOG.into() },
        |e| matches!(e, Event::Sources(s) if !s.plugins.iter().any(|p| p.id == CATALOG)),
    );
    let (fourth, path) = marker(4);
    let env = env.restart_with(bundled(vec![catalog_v2.clone(), path]));
    started(&env, &fourth);
    assert!(!env.sources().plugins.iter().any(|p| p.id == CATALOG));

    // Until they install it themselves: then bundled updates apply again.
    env.run(Command::InstallPlugin { path: catalog_v1 }, |e| {
        has_plugin(e, CATALOG)
    });
    assert_eq!(env.summary(CATALOG).version, "1.0.0");
    let (fifth, path) = marker(5);
    let env = env.restart_with(bundled(vec![catalog_v2, path]));
    started(&env, &fifth);
    assert_eq!(env.summary(CATALOG).version, "1.1.0");
}

#[test]
fn a_logout_sent_just_before_quitting_still_deletes_the_secrets() {
    let env = Env::new();
    // The service takes its time to revoke the token.
    let server = TestServer::start(|request| match request.path() {
        "/logout" => Response {
            delay: Duration::from_millis(800),
            ..Response::new(200, "bye")
        },
        _ => Response::new(404, "not found"),
    });
    env.plugin("echo", &echo_manifest(), ECHO, &server.url());
    env.core.send(Command::BeginLogin {
        plugin: ECHO.into(),
        method: AuthMethod::Qr,
    });
    auth_steps(&env, ECHO, |s| matches!(s, AuthStep::Done { .. }));
    let ours = |keys: Vec<String>| {
        keys.into_iter()
            .filter(|k| k.starts_with(&format!("jellyfesh/{ECHO}/")))
            .count()
    };
    assert!(ours(env.secrets.keys()) > 0);

    let Env {
        core,
        events,
        secrets,
        ..
    } = env;
    let logout = core.send(Command::Logout { plugin: ECHO.into() });
    core.shutdown();
    assert_eq!(ours(secrets.keys()), 0, "{:?}", secrets.keys());
    assert_eq!(server.count("/logout"), 1);
    // Its result was still reported.
    let reported: Vec<Event> = events.try_iter().collect();
    assert!(
        !reported
            .iter()
            .any(|e| matches!(e, Event::CommandFailed { id, .. } if *id == logout))
    );
    assert!(
        reported
            .iter()
            .any(|e| plugin_in(e, ECHO).is_some_and(|p| p.account.is_none()))
    );
}
