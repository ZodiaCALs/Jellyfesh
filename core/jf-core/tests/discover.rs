//! Discover pages, details, comments and source pictures through the core: the discover test
//! plugin (invented shelves, notes and comments, pictures that need a header), a plugin without
//! a Discover page of its own, and a library of generated tones with tags written here.

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use common::*;
use jf_core::{
    ArtworkRef, Command, Event, FactKey, ItemKind, ItemRef, QualityLabel, SecretStore, ShelfStyle, Status,
    TrackQuery, TrackRef,
};
use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::tag::{Accessor, ItemKey, Tag, TagExt};
use serde_json::json;

const DISCOVER: &str = "org.jellyfesh.test.discover";

const DISCOVER_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "server_url": { "type": "string", "format": "uri" },
        "hostile": { "type": "boolean", "default": false }
    }
}"#;

fn discover_manifest() -> String {
    format!(
        "id = {DISCOVER:?}\nname = \"Test Discover\"\nversion = \"1.0.0\"\napi_version = \"1.2\"\n\
         author = \"Jellyfesh tests\"\ncapabilities = [\"browse\", \"discover\", \"details\", \"comments\"]\n\
         playback_mode = \"stream\"\n\
         \n[network]\nallow = [\"${{settings.server_url}}\"]\n\
         \n[settings]\nschema = \"settings.schema.json\"\n\
         \n[content_policy]\nattribution = \"Invented Service\"\n"
    )
}

/// The service's pictures: a red square for anyone who sends the header, 403 for the rest.
fn picture_server() -> TestServer {
    TestServer::start(|req| {
        if !req.path().starts_with("/art/") {
            return Response::new(404, "");
        }
        if req.header("x-art-key") != Some("test-key") {
            return Response::new(403, "the key is missing");
        }
        Response::new(200, png([200, 30, 30], 300)).header("content-type", "image/png")
    })
}

fn discover_env(server: &TestServer) -> Env {
    let env = Env::new();
    env.install_with_schema("discover", &discover_manifest(), DISCOVER, DISCOVER_SCHEMA);
    env.settings(DISCOVER, json!({ "server_url": server.url() }));
    env
}

fn last<T>(events: &[Event], pick: impl Fn(&Event) -> Option<T>) -> T {
    events.iter().rev().find_map(pick).expect("the answer")
}

#[test]
fn a_sources_discover_page_details_and_comments() {
    let server = picture_server();
    let env = discover_env(&server);
    let events = env.run(
        Command::Discover {
            source: DISCOVER.into(),
        },
        |e| matches!(e, Event::Discover(_)),
    );
    let page = last(&events, |e| match e {
        Event::Discover(p) => Some(p.clone()),
        _ => None,
    });
    assert_eq!(page.source, DISCOVER);
    assert_eq!(page.error, None);
    let ids: Vec<&str> = page.shelves.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["new", "top"]);
    let top = &page.shelves[1];
    assert_eq!((top.title.as_str(), top.style), ("Top Songs", ShelfStyle::Chart));
    assert_eq!(top.more.as_deref(), Some("chart:top"));
    let song = &top.items[0];
    assert_eq!(song.title, "Invented Song 1");
    assert_eq!(song.artist_id.as_deref(), Some("ar:1"), "Go to Artist");
    assert_eq!(song.album_id.as_deref(), Some("al:1"), "Go to Album");
    assert!(matches!(&song.track, Some(TrackRef::Source(t)) if t.id == "tr:1"));

    // See All browses the shelf's node.
    let events = env.run(
        Command::Browse {
            plugin: DISCOVER.into(),
            node: Some("chart:top".into()),
            page: 0,
        },
        |e| matches!(e, Event::BrowsePage(_)),
    );
    let browsed = last(&events, |e| match e {
        Event::BrowsePage(p) => Some(p.clone()),
        _ => None,
    });
    assert_eq!(browsed.items.len(), 8);

    // Details: the plugin's notes and facts, its attribution.
    let item = ItemRef::Source {
        plugin: DISCOVER.into(),
        item_kind: ItemKind::Track,
        id: "tr:1".into(),
    };
    let events = env.run(Command::Details { item: item.clone() }, |e| {
        matches!(e, Event::Details(_))
    });
    let details = last(&events, |e| match e {
        Event::Details(d) => Some(d.clone()),
        _ => None,
    });
    assert_eq!(details.item, item);
    assert_eq!(
        details.title, "Invented Song 1",
        "the listed item, passed back whole"
    );
    assert_eq!(
        details.description.as_deref(),
        Some("Invented notes about Invented Song 1.")
    );
    assert_eq!(details.attribution.as_deref(), Some("Invented Service"));
    assert_eq!(details.comment_count, Some(3));
    assert!(
        details
            .facts
            .iter()
            .any(|f| f.key == FactKey::Other { label: "Mood".into() } && f.value == "Calm")
    );
    assert!(matches!(&details.image, Some(ArtworkRef::Url(u)) if u.ends_with("/art/tr-1-large.png")));

    // Comments, a page at a time; hot ones come with the first.
    let events = env.run(
        Command::Comments {
            item: item.clone(),
            page: None,
        },
        |e| matches!(e, Event::Comments(_)),
    );
    let first = last(&events, |e| match e {
        Event::Comments(c) => Some(c.clone()),
        _ => None,
    });
    assert_eq!(first.hot.len(), 1);
    assert_eq!(first.comments.len(), 2);
    assert_eq!(first.total, Some(3));
    assert_eq!(first.next.as_deref(), Some("2"));
    assert_eq!(first.attribution.as_deref(), Some("Invented Service"));
    assert_eq!(first.comments[0].likes, Some(7));
    let events = env.run(
        Command::Comments {
            item: item.clone(),
            page: Some("2".into()),
        },
        |e| matches!(e, Event::Comments(c) if c.next.is_none()),
    );
    let second = last(&events, |e| match e {
        Event::Comments(c) => Some(c.clone()),
        _ => None,
    });
    assert_eq!(second.comments.len(), 1);
    assert!(second.hot.is_empty());
    // Library items have none.
    let events = env.run(
        Command::Comments {
            item: ItemRef::Track { id: 1 },
            page: None,
        },
        |e| matches!(e, Event::Comments(_)),
    );
    let none = last(&events, |e| match e {
        Event::Comments(c) => Some(c.clone()),
        _ => None,
    });
    assert!(none.error.is_some());
}

#[test]
fn source_pictures_come_through_the_core_with_their_headers() {
    let server = picture_server();
    let env = discover_env(&server);
    let events = env.run(
        Command::Discover {
            source: DISCOVER.into(),
        },
        |e| matches!(e, Event::Discover(_)),
    );
    let page = last(&events, |e| match e {
        Event::Discover(p) => Some(p.clone()),
        _ => None,
    });
    let Some(ArtworkRef::Url(url)) = page.shelves[0].items[0].artwork.clone() else {
        panic!("a picture URL");
    };
    // Asked for twice before it is there: fetched once.
    assert_eq!(env.core.source_artwork(&url, 100), None);
    assert_eq!(env.core.source_artwork(&url, 100), None);
    let events = wait_for(
        &env.events,
        Duration::from_secs(20),
        |e| matches!(e, Event::Artwork(a) if a.url == url),
    );
    let ready = last(&events, |e| match e {
        Event::Artwork(a) if a.url == url => Some(a.clone()),
        _ => None,
    });
    assert_eq!(ready.size_px, 100);
    let path = ready.path.clone().expect("fetched with the plugin's header");
    assert!(path.is_file());
    let jpeg = std::fs::read(&path).unwrap();
    assert_eq!(&jpeg[..2], &[0xff, 0xd8], "a JPEG of about the size asked for");
    assert_eq!(
        env.core.source_artwork(&url, 100).as_deref(),
        Some(path.as_path())
    );
    let path_of = url.trim_start_matches(&server.url()).to_owned();
    assert_eq!(server.count(&path_of), 1);
    assert_eq!(
        server.with_path(&path_of)[0].header("x-art-key"),
        Some("test-key")
    );

    // Another size is made from the kept picture.
    assert_eq!(env.core.source_artwork(&url, 600), None);
    wait_for(
        &env.events,
        Duration::from_secs(20),
        |e| matches!(e, Event::Artwork(a) if a.url == url && a.size_px == 600 && a.path.is_some()),
    );
    assert_eq!(server.count(&path_of), 1);

    // A picture no source gave is not fetched.
    let stranger = format!("{}/art/stranger.png", server.url());
    assert_eq!(env.core.source_artwork(&stranger, 64), None);
    wait_for(
        &env.events,
        Duration::from_secs(20),
        |e| matches!(e, Event::Artwork(a) if a.url == stranger && a.path.is_none()),
    );
    assert_eq!(server.count("/art/stranger.png"), 0);
}

#[test]
fn a_page_is_made_for_sources_without_their_own() {
    // The echo plugin lists a folder at its root and has recommendations, but no Discover page.
    let server = TestServer::start(|_| Response::new(404, ""));
    let env = Env::new();
    env.plugin("echo", &echo_manifest(), ECHO, &server.url());
    // Signed in, so For You has something.
    env.secrets
        .set(&format!("jellyfesh/{ECHO}/token"), "invented-token")
        .unwrap();
    let events = env.run(Command::Discover { source: ECHO.into() }, |e| {
        matches!(e, Event::Discover(_))
    });
    let page = last(&events, |e| match e {
        Event::Discover(p) => Some(p.clone()),
        _ => None,
    });
    assert_eq!(page.error, None);
    let ids: Vec<&str> = page.shelves.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["for_you", "albums"]);
    let for_you = &page.shelves[0];
    assert_eq!(for_you.title, "For You");
    assert_eq!(for_you.items[0].title, "Daily mix");
    let albums = &page.shelves[1];
    assert_eq!(albums.title, "Albums");
    assert_eq!(
        albums.more.as_deref(),
        Some("albums"),
        "See All browses the folder"
    );
    assert_eq!(albums.items.len(), 20);
    assert!(albums.items.iter().all(|i| i.kind == ItemKind::Album));
    // Signed out: the folders alone.
    env.secrets.delete(&format!("jellyfesh/{ECHO}/token")).unwrap();
    let events = env.run(Command::Discover { source: ECHO.into() }, |e| {
        matches!(e, Event::Discover(_))
    });
    let page = last(&events, |e| match e {
        Event::Discover(p) => Some(p.clone()),
        _ => None,
    });
    let ids: Vec<&str> = page.shelves.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["albums"]);
    // An unknown source says so.
    assert!(
        env.fails(Command::Discover {
            source: "org.nope".into()
        })
        .contains("not installed")
    );
}

/// Writes tags into a generated file.
fn tag(path: &Path, edit: impl FnOnce(&mut Tag)) {
    let mut file = lofty::read_from_path(path).unwrap();
    let tag_type = file.primary_tag_type();
    if file.tag(tag_type).is_none() {
        file.insert_tag(Tag::new(tag_type));
    }
    let tag = file.tag_mut(tag_type).unwrap();
    edit(tag);
    tag.save_to_path(path, WriteOptions::default()).unwrap();
}

/// A 24-bit 96 kHz stereo tone.
fn hi_res_wav(path: &Path, seconds: f32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 96_000,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..(96_000.0 * seconds) as u32 {
        let v = ((std::f32::consts::TAU * 440.0 * i as f32 / 96_000.0).sin() * 0.25 * 8_388_607.0) as i32;
        w.write_sample(v).unwrap();
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
}

fn library(env: &Env) -> (PathBuf, PathBuf, PathBuf) {
    let music = env.dir.path().join("music");
    std::fs::create_dir_all(music.join("album")).unwrap();
    let flac = music.join("album/invented.flac");
    std::fs::write(&flac, sine_flac(440.0, 1.0)).unwrap();
    tag(&flac, |t| {
        t.set_title("Invented Song".into());
        t.set_artist("Invented Artist".into());
        t.set_album("Invented Album".into());
        t.set_track(3);
        t.insert_text(ItemKey::TrackTotal, "12".into());
        t.insert_text(ItemKey::Composer, "An Invented Composer".into());
        t.insert_text(ItemKey::Label, "Invented Label".into());
        t.insert_text(ItemKey::Genre, "Invented Pop".into());
    });
    let wav = music.join("album/tone.wav");
    sine_wav(&wav, 330.0, 1.0);
    let hi_res = env.dir.path().join("hi-res.wav");
    hi_res_wav(&hi_res, 1.0);
    env.run(Command::AddRoot { path: music }, |e| {
        matches!(e, Event::Scan(jf_core::ScanStatus::Finished(_)))
    });
    (flac, wav, hi_res)
}

#[test]
fn library_details_discover_and_quality() {
    let env = Env::new();
    let (flac, wav, hi_res) = library(&env);
    let tracks = env.core.tracks(&TrackQuery::default()).unwrap();
    let song = tracks.iter().find(|t| t.path == flac).expect("scanned");

    // About This Song: the tags and the file.
    let events = env.run(
        Command::Details {
            item: ItemRef::Track { id: song.id },
        },
        |e| matches!(e, Event::Details(_)),
    );
    let details = last(&events, |e| match e {
        Event::Details(d) => Some(d.clone()),
        _ => None,
    });
    assert_eq!(details.title, "Invented Song");
    assert_eq!(details.subtitle.as_deref(), Some("Invented Artist"));
    let fact = |key: FactKey| {
        details
            .facts
            .iter()
            .find(|f| f.key == key)
            .map(|f| f.value.clone())
    };
    assert_eq!(fact(FactKey::Composer).as_deref(), Some("An Invented Composer"));
    assert_eq!(fact(FactKey::Label).as_deref(), Some("Invented Label"));
    assert_eq!(fact(FactKey::Track).as_deref(), Some("3/12"));
    let technical = details.technical.clone().expect("technical information");
    assert_eq!(technical.format.codec.as_deref(), Some("flac"));
    assert_eq!(technical.format.sample_rate_hz, Some(RATE));
    assert_eq!(technical.format.bit_depth, Some(16));
    assert_eq!(technical.format.quality, Some(QualityLabel::Lossless));
    assert_eq!(technical.container.as_deref(), Some("flac"));
    assert_eq!(technical.path.as_deref(), Some(flac.as_path()));
    assert!(technical.file_size_bytes.is_some_and(|b| b > 0));

    // A file outside the library, and its quality.
    let events = env.run(
        Command::Details {
            item: ItemRef::File { path: hi_res.clone() },
        },
        |e| matches!(e, Event::Details(_)),
    );
    let details = last(&events, |e| match e {
        Event::Details(d) => Some(d.clone()),
        _ => None,
    });
    assert_eq!(details.title, "hi-res", "untagged: the file's name");
    let technical = details.technical.clone().unwrap();
    assert_eq!(technical.format.bit_depth, Some(24));
    assert_eq!(technical.format.quality, Some(QualityLabel::HiResLossless));

    // Albums and artists from the catalogue.
    let album = song.album_id.expect("grouped into an album");
    let events = env.run(
        Command::Details {
            item: ItemRef::Album { id: album },
        },
        |e| matches!(e, Event::Details(_)),
    );
    let details = last(&events, |e| match e {
        Event::Details(d) => Some(d.clone()),
        _ => None,
    });
    assert_eq!(details.title, "Invented Album");
    assert!(
        details
            .facts
            .iter()
            .any(|f| f.key == FactKey::Genre && f.value == "Invented Pop")
    );
    assert!(
        env.fails(Command::Details {
            item: ItemRef::Album { id: 999_999 }
        })
        .contains("not in the library")
    );

    // Now Playing says what is decoded: the engine's format and the label.
    env.run(
        Command::PlayTracks {
            tracks: vec![TrackRef::File(hi_res.clone())],
            start: 0,
        },
        |e| matches!(e, Event::NowPlaying(Some(n)) if n.quality.is_some()),
    );
    let now = env.core.snapshot().now_playing.clone().unwrap();
    let quality = now.quality.clone().unwrap();
    assert_eq!(quality.quality, Some(QualityLabel::HiResLossless));
    assert_eq!(quality.sample_rate_hz, Some(96_000));
    assert_eq!(quality.bit_depth, Some(24));
    assert_eq!(quality.codec.as_deref(), Some("pcm"));

    // Played to the end, the songs count for Most Played; songs played often long ago come back
    // in Rediscover.
    env.run(
        Command::PlayTracks {
            tracks: vec![TrackRef::Library(song.id)],
            start: 0,
        },
        |e| matches!(e, Event::Playback(p) if p.status == Status::Ended),
    );
    let old = jf_library::Library::open(
        &env.dir.path().join("data/library.sqlite"),
        &env.dir.path().join("cache"),
    )
    .unwrap();
    let tone = tracks.iter().find(|t| t.path == wav).unwrap();
    let long_ago = 1_600_000_000_000;
    for _ in 0..2 {
        old.record_play(&jf_library::PlaylistItem::Library(tone.id), long_ago, true)
            .unwrap();
    }
    let events = env.run(
        Command::Discover {
            source: "library".into(),
        },
        |e| matches!(e, Event::Discover(_)),
    );
    let page = last(&events, |e| match e {
        Event::Discover(p) => Some(p.clone()),
        _ => None,
    });
    assert_eq!(page.source, "library");
    let shelf = |id: &str| page.shelves.iter().find(|s| s.id == id).cloned();
    let added = shelf("recently_added").expect("Recently Added");
    // The album of the tagged song, and the untagged tone on its own.
    assert!(
        added
            .items
            .iter()
            .any(|i| i.kind == ItemKind::Album && i.title == "Invented Album")
    );
    assert!(
        added
            .items
            .iter()
            .any(|i| i.kind == ItemKind::Track && i.title == "tone")
    );
    let most = shelf("most_played").expect("Most Played");
    assert_eq!(most.style, ShelfStyle::Chart);
    let titles: Vec<&str> = most.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(titles, ["tone", "Invented Song"], "by plays: two, then one");
    let again = shelf("rediscover").expect("Rediscover");
    assert!(again.items.iter().any(|i| i.id == tone.id.to_string()));
}
