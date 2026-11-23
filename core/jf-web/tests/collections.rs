//! Hearts, playlists and plays (`jf_web::collections`) mirror `jf-library`'s
//! rules, which need SQLite and do not build for the browser: the same steps
//! must give the same lists in both. Runs on the host only.

#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;

use jf_library::{Library, PlaylistItem};
use jf_plugin_model::{Item, ItemKind};
use jf_web::collections::{Collections, Saved};

const PLUGIN: &str = "org.example.p";

/// A library in a folder of its own, removed afterwards.
struct Native {
    dir: PathBuf,
    lib: Library,
}

impl Native {
    fn new(name: &str) -> Native {
        let dir = std::env::temp_dir().join(format!("jf-web-collections-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let lib = Library::open(&dir.join("library.db"), &dir.join("cache")).unwrap();
        Native { dir, lib }
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn duration(id: &str) -> u32 {
    1_000 * (10 + id.bytes().map(u32::from).sum::<u32>() % 50)
}

fn web_item(id: &str) -> Saved {
    Saved {
        plugin: PLUGIN.into(),
        item: Item {
            id: id.into(),
            kind: ItemKind::Track,
            title: format!("Song {id}"),
            artist: None,
            album: None,
            album_id: None,
            artist_id: None,
            duration_ms: Some(duration(id)),
            track_number: None,
            disc_number: None,
            year: None,
            child_count: None,
            isrc: None,
            artwork: None,
            explicit: false,
            playable: true,
            unplayable: None,
        },
    }
}

fn native_item(id: &str) -> PlaylistItem {
    PlaylistItem::Source {
        plugin: PLUGIN.into(),
        id: id.into(),
        meta_json: format!(r#"{{"duration_ms": {}}}"#, duration(id)),
    }
}

fn native_id(item: &PlaylistItem) -> String {
    match item {
        PlaylistItem::Source { id, .. } => id.clone(),
        other => panic!("unexpected item {other:?}"),
    }
}

#[test]
fn playlists_list_in_the_same_order() {
    let native = Native::new("names");
    let mut ours = Collections::new();
    let names = [
        "beach",
        "Ärger",
        "Beach",
        "  zebra ",
        "äpfel",
        "Zoo",
        "10 Songs",
        "2 Songs",
        "ＷＩＤＥ",
    ];
    for name in names {
        native.lib.create_playlist(name).unwrap();
        ours.create_playlist(name, 1).unwrap();
    }
    assert!(native.lib.create_playlist("  ").is_err());
    assert!(ours.create_playlist("  ", 1).is_err());
    let theirs: Vec<String> = native
        .lib
        .playlists()
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    let mine: Vec<String> = ours.playlists().into_iter().map(|p| p.name).collect();
    assert_eq!(mine, theirs);
}

#[test]
fn entries_change_in_the_same_order() {
    let native = Native::new("entries");
    let mut ours = Collections::new();
    let theirs_id = native.lib.create_playlist("Walk").unwrap();
    let mine_id = ours.create_playlist("Walk", 1).unwrap();
    let mut theirs_entries = Vec::new();
    let mut mine_entries = Vec::new();
    // Each step on both sides: (songs, position), then moves and removals by index.
    for (songs, at) in [
        (vec!["a", "b", "c"], None),
        (vec!["d"], Some(1)),
        (vec!["e", "a"], Some(0)),
        (vec!["f"], Some(99)),
        (vec![], Some(2)),
    ] {
        let native_items: Vec<PlaylistItem> = songs.iter().map(|s| native_item(s)).collect();
        let web_items: Vec<Saved> = songs.iter().map(|s| web_item(s)).collect();
        theirs_entries.extend(native.lib.add_to_playlist(theirs_id, &native_items, at).unwrap());
        mine_entries.extend(ours.add_to_playlist(mine_id, web_items, at, 2).unwrap());
    }
    let compare = |ours: &Collections| {
        let theirs: Vec<String> = native
            .lib
            .playlist_entries(theirs_id, 0, 100)
            .unwrap()
            .iter()
            .map(|e| native_id(&e.item))
            .collect();
        let mine: Vec<String> = ours
            .entries(mine_id, 0, 100)
            .into_iter()
            .map(|e| e.item.id)
            .collect();
        assert_eq!(mine, theirs);
        let (theirs, mine) = (
            native.lib.playlist(theirs_id).unwrap().unwrap(),
            ours.playlist(mine_id).unwrap(),
        );
        assert_eq!(mine.entry_count as u32, theirs.entry_count);
        assert_eq!(mine.duration_ms, theirs.duration_ms);
    };
    compare(&ours);
    // Moves by entry (the n-th added), to places inside and past the end.
    for (nth, to) in [(0, 3), (5, 0), (2, 99), (6, 1), (1, 1)] {
        native
            .lib
            .move_playlist_entry(theirs_id, theirs_entries[nth], to)
            .unwrap();
        ours.move_playlist_entry(mine_id, mine_entries[nth], to, 3)
            .unwrap();
        compare(&ours);
    }
    native
        .lib
        .remove_from_playlist(theirs_id, &[theirs_entries[3], theirs_entries[0], 9_999])
        .unwrap();
    ours.remove_from_playlist(mine_id, &[mine_entries[3], mine_entries[0], 9_999], 4)
        .unwrap();
    compare(&ours);
    assert!(native.lib.move_playlist_entry(theirs_id, 9_999, 0).is_err());
    assert!(ours.move_playlist_entry(mine_id, 9_999, 0, 5).is_err());
}

#[test]
fn favourites_and_plays_list_in_the_same_order() {
    let native = Native::new("stats");
    let mut ours = Collections::new();
    // Hearts in a burst (one millisecond, as clicks in a test are), some given twice or taken back.
    let hearts = [
        ("a", true),
        ("b", true),
        ("c", true),
        ("a", true),
        ("b", false),
        ("d", true),
        ("b", true),
    ];
    for (id, on) in hearts {
        native.lib.set_favorite(&native_item(id), on).unwrap();
        ours.set_favorite(web_item(id), on, 1_000).unwrap();
    }
    let theirs: Vec<String> = native
        .lib
        .favorites(0, 100)
        .unwrap()
        .iter()
        .map(|s| native_id(&s.item))
        .collect();
    let mine: Vec<String> = ours.favorites(0, 100).into_iter().map(|s| s.item.id).collect();
    assert_eq!(mine, theirs);
    for (id, at, counted) in [
        ("a", 5_000, true),
        ("e", 6_000, true),
        ("e", 7_000, false),
        ("f", 7_000, true),
        ("a", 4_000, true),
        ("g", 8_000, false),
        ("f", 9_000, true),
        ("h", 9_000, true),
    ] {
        native.lib.record_play(&native_item(id), at, counted).unwrap();
        ours.record_play(web_item(id), at, counted).unwrap();
    }
    let rows = |list: Vec<jf_library::ItemStats>| -> Vec<(String, u64, u64, Option<i64>)> {
        list.iter()
            .map(|s| (native_id(&s.item), s.play_count, s.skip_count, s.last_played_ms))
            .collect()
    };
    let web_rows = |list: Vec<jf_web::collections::ItemStats>| -> Vec<(String, u64, u64, Option<i64>)> {
        list.into_iter()
            .map(|s| (s.item.id, s.play_count, s.skip_count, s.last_played_ms))
            .collect()
    };
    assert_eq!(
        web_rows(ours.recently_played(100)),
        rows(native.lib.recently_played(100).unwrap())
    );
    assert_eq!(
        web_rows(ours.most_played(100)),
        rows(native.lib.most_played(100).unwrap())
    );
    // Taking a heart back from a played song keeps its plays.
    native.lib.set_favorite(&native_item("a"), false).unwrap();
    ours.set_favorite(web_item("a"), false, 2_000).unwrap();
    assert_eq!(
        web_rows(ours.most_played(100)),
        rows(native.lib.most_played(100).unwrap())
    );
    assert_eq!(ours.favorite_count(), native.lib.favorites(0, 100).unwrap().len());
}
