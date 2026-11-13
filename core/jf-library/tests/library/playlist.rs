//! Playlists: create, rename, delete; entry order; tracks, files and plugin
//! items; relinking after a rescan; M3U8 import and export.

use std::fs;
use std::path::{Path, PathBuf};

use jf_library::{LibraryError, PlaylistEntry, PlaylistItem};

use crate::fixtures::{self, Fixture, standard};

fn source(id: &str) -> PlaylistItem {
    PlaylistItem::Source {
        plugin: "org.example.test".to_owned(),
        id: id.to_owned(),
        meta_json: String::new(),
    }
}

fn entries(fx: &Fixture, id: i64) -> Vec<PlaylistEntry> {
    fx.lib.playlist_entries(id, 0, 1000).unwrap()
}

/// The ids of the plugin items, in playlist order; positions must run 0..n.
fn order(fx: &Fixture, id: i64) -> Vec<String> {
    let entries = entries(fx, id);
    let positions: Vec<usize> = entries.iter().map(|e| e.position).collect();
    assert_eq!(
        positions,
        (0..entries.len()).collect::<Vec<_>>(),
        "gap-free positions"
    );
    assert_eq!(
        fx.lib.playlist(id).unwrap().unwrap().entry_count as usize,
        entries.len()
    );
    entries
        .into_iter()
        .map(|e| match e.item {
            PlaylistItem::Source { id, .. } => id,
            other => panic!("{other:?}"),
        })
        .collect()
}

fn entry_id(fx: &Fixture, id: i64, name: &str) -> i64 {
    entries(fx, id)
        .into_iter()
        .find(|e| e.item == source(name))
        .unwrap_or_else(|| panic!("no entry {name}"))
        .entry_id
}

#[test]
fn playlists_are_created_renamed_listed_and_deleted() {
    let fx = Fixture::new();
    let start = fx.lib.revision().unwrap();
    let road = fx.lib.create_playlist("  Road trip ").unwrap();
    let ambient = fx.lib.create_playlist("ambient").unwrap();
    let again = fx.lib.create_playlist("Road trip").unwrap();
    assert_eq!(fx.lib.revision().unwrap(), start + 3);

    let names: Vec<String> = fx.lib.playlists().unwrap().into_iter().map(|p| p.name).collect();
    assert_eq!(names, ["ambient", "Road trip", "Road trip"], "by name, then id");
    let empty = fx.lib.playlist(road).unwrap().unwrap();
    assert_eq!(empty.name, "Road trip", "trimmed");
    assert_eq!(
        (empty.entry_count, empty.duration_ms, empty.artwork_hash),
        (0, 0, None)
    );
    assert!(empty.created_at_ms > 1_700_000_000_000);
    assert_eq!(empty.updated_at_ms, empty.created_at_ms);
    assert_eq!(fx.lib.playlist(-1).unwrap(), None);

    let before = fx.lib.revision().unwrap();
    fx.lib.rename_playlist(road, "Bus ride").unwrap();
    assert_eq!(fx.lib.revision().unwrap(), before + 1);
    let renamed = fx.lib.playlist(road).unwrap().unwrap();
    assert_eq!(renamed.name, "Bus ride");
    assert!(renamed.updated_at_ms >= empty.updated_at_ms);
    fx.lib.rename_playlist(road, " Bus ride").unwrap();
    assert_eq!(
        fx.lib.revision().unwrap(),
        before + 1,
        "the same name is no change"
    );

    for err in [
        fx.lib.create_playlist(" \t").unwrap_err(),
        fx.lib.rename_playlist(road, "").unwrap_err(),
    ] {
        assert!(matches!(err, LibraryError::EmptyPlaylistName), "{err}");
    }
    let err = fx.lib.rename_playlist(999, "x").unwrap_err();
    assert!(matches!(err, LibraryError::PlaylistNotFound(999)), "{err}");

    fx.lib.add_to_playlist(ambient, &[source("1")], None).unwrap();
    fx.lib.delete_playlist(ambient).unwrap();
    assert_eq!(fx.lib.playlist(ambient).unwrap(), None);
    assert!(fx.lib.playlist_entries(ambient, 0, 10).unwrap().is_empty());
    let err = fx.lib.delete_playlist(ambient).unwrap_err();
    assert!(matches!(err, LibraryError::PlaylistNotFound(_)), "{err}");

    let newer = fx.lib.create_playlist("ambient").unwrap();
    assert!(newer > again, "ids are never reused");
    assert_eq!(fx.lib.playlists().unwrap().len(), 3);
}

#[test]
fn entries_keep_their_order_through_inserts_moves_and_removals() {
    let fx = Fixture::new();
    let id = fx.lib.create_playlist("Order").unwrap();
    let ids = fx
        .lib
        .add_to_playlist(id, &[source("1"), source("2"), source("3")], None)
        .unwrap();
    assert_eq!(ids.len(), 3);
    assert_eq!(order(&fx, id), ["1", "2", "3"]);
    let first = entries(&fx, id);
    assert_eq!(
        first.iter().map(|e| e.entry_id).collect::<Vec<_>>(),
        ids,
        "ids in the order given"
    );

    fx.lib.add_to_playlist(id, &[source("0")], Some(0)).unwrap();
    fx.lib.add_to_playlist(id, &[source("9")], Some(99)).unwrap();
    fx.lib
        .add_to_playlist(id, &[source("5"), source("6")], Some(3))
        .unwrap();
    assert_eq!(order(&fx, id), ["0", "1", "2", "5", "6", "3", "9"]);
    let revision = fx.lib.revision().unwrap();
    assert!(fx.lib.add_to_playlist(id, &[], Some(0)).unwrap().is_empty());
    assert_eq!(
        fx.lib.revision().unwrap(),
        revision,
        "adding nothing is no change"
    );

    let zero = entry_id(&fx, id, "0");
    fx.lib.move_playlist_entry(id, zero, 6).unwrap();
    assert_eq!(
        order(&fx, id),
        ["1", "2", "5", "6", "3", "9", "0"],
        "first to last"
    );
    fx.lib.move_playlist_entry(id, zero, 0).unwrap();
    assert_eq!(
        order(&fx, id),
        ["0", "1", "2", "5", "6", "3", "9"],
        "last to first"
    );
    fx.lib
        .move_playlist_entry(id, entry_id(&fx, id, "3"), 100)
        .unwrap();
    assert_eq!(
        order(&fx, id),
        ["0", "1", "2", "5", "6", "9", "3"],
        "past the end"
    );
    fx.lib.move_playlist_entry(id, entry_id(&fx, id, "5"), 1).unwrap();
    assert_eq!(order(&fx, id), ["0", "5", "1", "2", "6", "9", "3"], "backwards");
    fx.lib.move_playlist_entry(id, entry_id(&fx, id, "1"), 4).unwrap();
    assert_eq!(order(&fx, id), ["0", "5", "2", "6", "1", "9", "3"], "forwards");
    let revision = fx.lib.revision().unwrap();
    fx.lib.move_playlist_entry(id, entry_id(&fx, id, "1"), 4).unwrap();
    assert_eq!(
        fx.lib.revision().unwrap(),
        revision,
        "moving in place is no change"
    );

    let other = fx.lib.create_playlist("Other").unwrap();
    let foreign = fx.lib.add_to_playlist(other, &[source("x")], None).unwrap()[0];
    let gone = [entry_id(&fx, id, "1"), entry_id(&fx, id, "9")];
    fx.lib
        .remove_from_playlist(id, &[gone[0], foreign, gone[1], 12_345, gone[0]])
        .unwrap();
    assert_eq!(order(&fx, id), ["0", "5", "2", "6", "3"]);
    assert_eq!(order(&fx, other), ["x"], "another playlist's entry is left alone");
    let revision = fx.lib.revision().unwrap();
    fx.lib.remove_from_playlist(id, &[gone[0]]).unwrap();
    assert_eq!(
        fx.lib.revision().unwrap(),
        revision,
        "removing nothing is no change"
    );

    let page = fx.lib.playlist_entries(id, 2, 2).unwrap();
    assert_eq!(
        page.iter()
            .map(|e| (e.position, e.item.clone()))
            .collect::<Vec<_>>(),
        [(2, source("2")), (3, source("6"))]
    );
    assert!(fx.lib.playlist_entries(id, 5, 10).unwrap().is_empty());
    assert!(fx.lib.playlist_entries(id, 0, 0).unwrap().is_empty());

    let added = fx.lib.add_to_playlist(id, &[source("7")], Some(1)).unwrap()[0];
    assert!(added > foreign, "entry ids are never reused");
    assert_eq!(order(&fx, id), ["0", "7", "5", "2", "6", "3"]);

    let err = fx.lib.move_playlist_entry(id, foreign, 0).unwrap_err();
    assert!(
        matches!(err, LibraryError::PlaylistEntryNotFound { entry_id, .. } if entry_id == foreign),
        "{err}"
    );
    for err in [
        fx.lib.add_to_playlist(-1, &[source("1")], None).unwrap_err(),
        fx.lib.remove_from_playlist(-1, &[added]).unwrap_err(),
        fx.lib.move_playlist_entry(-1, added, 0).unwrap_err(),
    ] {
        assert!(matches!(err, LibraryError::PlaylistNotFound(-1)), "{err}");
    }
}

#[test]
fn entries_resolve_tracks_files_and_plugin_items() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let a = fx.track_at(standard::COVER_A);
    let cjk = fx.track_at(standard::CJK);
    let loose = fx.dir.path().join("loose.wav");
    fixtures::wav(&loose, 100);
    let remote = PlaylistItem::Source {
        plugin: "org.jellyfesh.subsonic".to_owned(),
        id: "tr:1".to_owned(),
        meta_json: r#"{"title":"Remote","duration_ms":61000}"#.to_owned(),
    };
    let unparsable = PlaylistItem::Source {
        plugin: "org.jellyfesh.subsonic".to_owned(),
        id: "tr:2".to_owned(),
        meta_json: "not json".to_owned(),
    };

    let id = fx.lib.create_playlist("Mixed").unwrap();
    let revision = fx.lib.revision().unwrap();
    fx.lib
        .add_to_playlist(
            id,
            &[
                PlaylistItem::File(fx.path(standard::CJK)),
                PlaylistItem::Library(a.id),
                PlaylistItem::File(loose.clone()),
                remote.clone(),
                unparsable.clone(),
                PlaylistItem::Library(a.id),
            ],
            None,
        )
        .unwrap();
    assert_eq!(fx.lib.revision().unwrap(), revision + 1);

    let got = entries(&fx, id);
    let items: Vec<PlaylistItem> = got.iter().map(|e| e.item.clone()).collect();
    assert_eq!(
        items,
        [
            PlaylistItem::File(fx.path(standard::CJK)),
            PlaylistItem::Library(a.id),
            PlaylistItem::File(loose),
            remote,
            unparsable,
            PlaylistItem::Library(a.id),
        ]
    );
    let tracks: Vec<Option<&jf_library::Track>> = got.iter().map(|e| e.track.as_ref()).collect();
    assert_eq!(
        tracks,
        [Some(&cjk), Some(&a), None, None, None, Some(&a)],
        "a file in the catalogue resolves like a library entry"
    );

    let playlist = fx.lib.playlist(id).unwrap().unwrap();
    assert_eq!(playlist.entry_count, 6);
    assert_eq!(
        playlist.duration_ms,
        cjk.duration_ms.unwrap() + 2 * a.duration_ms.unwrap() + 61_000,
        "tracks and plugin metadata; unparsable metadata counts nothing"
    );
    assert!(cjk.artwork_hash.is_none());
    assert_eq!(playlist.artwork_hash, a.artwork_hash, "first entry with artwork");
    assert!(playlist.updated_at_ms >= playlist.created_at_ms);

    let err = fx
        .lib
        .add_to_playlist(
            id,
            &[PlaylistItem::Library(a.id), PlaylistItem::Library(-5)],
            None,
        )
        .unwrap_err();
    assert!(matches!(err, LibraryError::TrackNotFound(-5)), "{err}");
    assert_eq!(entries(&fx, id).len(), 6, "all or nothing");
}

#[test]
fn library_entries_find_their_track_again_after_a_rescan() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let a = fx.track_at(standard::COVER_A);
    let b = fx.track_at(standard::COVER_B);
    let id = fx.lib.create_playlist("Keep").unwrap();
    fx.lib
        .add_to_playlist(
            id,
            &[PlaylistItem::Library(a.id), PlaylistItem::Library(b.id)],
            None,
        )
        .unwrap();

    let away = fx.dir.path().join("away.flac");
    fs::rename(fx.path(standard::COVER_A), &away).unwrap();
    assert_eq!(fx.scan().removed, 1);
    let got = entries(&fx, id);
    assert_eq!(got.len(), 2, "the entry stays");
    assert_eq!(
        got[0].item,
        PlaylistItem::File(fx.path(standard::COVER_A)),
        "as the file it was, not by an id another track may take"
    );
    assert_eq!(got[0].track, None);
    assert_eq!(got[1].track.as_ref(), Some(&b));
    assert_eq!(
        fx.lib.playlist(id).unwrap().unwrap().duration_ms,
        b.duration_ms.unwrap()
    );

    fs::rename(&away, fx.path(standard::COVER_A)).unwrap();
    assert_eq!(fx.scan().added, 1);
    let back = fx.track_at(standard::COVER_A);
    assert_ne!(back.id, a.id, "scanned in as a new track");
    let got = entries(&fx, id);
    assert_eq!(got[0].item, PlaylistItem::Library(back.id));
    assert_eq!(got[0].track.as_ref(), Some(&back));
    assert_eq!(
        fx.lib.playlist(id).unwrap().unwrap().artwork_hash,
        back.artwork_hash
    );
}

/// Tracks with non-ASCII names, plus one whose relative path starts with '#'.
fn named_library(fx: &Fixture) -> Vec<(PathBuf, &'static str)> {
    let files = [
        ("周杰伦/晴天.wav", "晴天", "周杰伦"),
        ("Ñandú/Café del Mar.flac", "Café del Mar", "Ñandú"),
        ("#1 hits/first.wav", "First", "Band"),
    ];
    let mut out = Vec::new();
    for (relative, title, artist) in files {
        let path = fx.path(relative);
        if relative.ends_with(".flac") {
            fixtures::flac(&path, 1600);
        } else {
            fixtures::wav(&path, 1600);
        }
        fixtures::tag(&path, |t| fixtures::basic(t, title, artist, "Names", 1));
        out.push((path, relative));
    }
    assert_eq!(fx.scan().added, 3);
    out
}

#[test]
fn m3u_export_and_import_round_trip_with_non_ascii_names() {
    let fx = Fixture::new();
    let tracks: Vec<_> = named_library(&fx)
        .iter()
        .map(|(_, relative)| fx.track_at(relative))
        .collect();
    let outside = fx.dir.path().join("elsewhere").join("Été.wav");
    fixtures::wav(&outside, 100);

    let id = fx.lib.create_playlist("Exported").unwrap();
    fx.lib
        .add_to_playlist(
            id,
            &[
                PlaylistItem::Library(tracks[0].id),
                PlaylistItem::Library(tracks[1].id),
                source("not a file"),
                PlaylistItem::File(outside.clone()),
                PlaylistItem::Library(tracks[2].id),
            ],
            None,
        )
        .unwrap();

    let m3u = fx.music.join("list.m3u8");
    fx.lib.export_m3u(id, &m3u).unwrap();
    let text = fs::read_to_string(&m3u).unwrap();
    let relative = |parts: &[&str]| parts.iter().collect::<PathBuf>().to_str().unwrap().to_owned();
    let expected = [
        "#EXTM3U".to_owned(),
        "#EXTINF:2,周杰伦 - 晴天".to_owned(),
        relative(&["周杰伦", "晴天.wav"]),
        "#EXTINF:2,Ñandú - Café del Mar".to_owned(),
        relative(&["Ñandú", "Café del Mar.flac"]),
        "#EXTINF:-1,Été".to_owned(),
        outside.to_str().unwrap().to_owned(),
        "#EXTINF:2,Band - First".to_owned(),
        relative(&[".", "#1 hits", "first.wav"]),
    ];
    assert_eq!(text.lines().collect::<Vec<_>>(), expected);
    assert!(text.ends_with('\n'));

    let imported = fx.lib.import_m3u(&m3u, "Imported").unwrap();
    let items = |id: i64| -> Vec<(PlaylistItem, Option<jf_library::Track>)> {
        entries(&fx, id).into_iter().map(|e| (e.item, e.track)).collect()
    };
    let mut original = items(id);
    original.retain(|(item, _)| !matches!(item, PlaylistItem::Source { .. }));
    assert_eq!(items(imported), original);
    assert_eq!(fx.lib.playlist(imported).unwrap().unwrap().name, "Imported");

    // Exported beside the music rather than above it, every path is absolute.
    let lists = fx.dir.path().join("lists");
    fs::create_dir_all(&lists).unwrap();
    let beside = lists.join("beside.m3u8");
    fx.lib.export_m3u(id, &beside).unwrap();
    let text = fs::read_to_string(&beside).unwrap();
    assert!(
        text.lines()
            .filter(|line| !line.starts_with('#'))
            .all(|line| Path::new(line).is_absolute()),
        "{text}"
    );
    let again = fx.lib.import_m3u(&beside, "Again").unwrap();
    assert_eq!(items(again), original);

    let err = fx.lib.export_m3u(-1, &beside).unwrap_err();
    assert!(matches!(err, LibraryError::PlaylistNotFound(-1)), "{err}");
}

#[test]
fn m3u_import_reads_relative_paths_urls_comments_and_byte_order_marks() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let rg = fx.track_at(standard::RG);
    let a = fx.track_at(standard::COVER_A);
    let lists = fx.dir.path().join("lists");
    fs::create_dir_all(&lists).unwrap();

    let mut lines = vec![
        "#EXTM3U".to_owned(),
        "#EXTINF:3,Alpha - Gain Check".to_owned(),
        "../music/wav/replaygain.wav".to_owned(),
        String::new(),
        "# a comment".to_owned(),
        "https://example.com/stream.mp3".to_owned(),
        "  missing/gone.flac  ".to_owned(),
    ];
    if cfg!(unix) {
        let url = format!(
            "file://{}",
            fx.path(standard::COVER_A).to_str().unwrap().replace(' ', "%20")
        );
        lines.push(url);
    }
    let body = lines.join("\r\n");
    let utf8 = lists.join("mix.m3u");
    fs::write(&utf8, format!("\u{feff}{body}\r\n")).unwrap();
    let mut utf16 = vec![0xFF, 0xFE];
    utf16.extend(body.encode_utf16().flat_map(u16::to_le_bytes));
    let utf16_path = lists.join("mix16.m3u");
    fs::write(&utf16_path, utf16).unwrap();

    let mut expected = vec![
        (PlaylistItem::Library(rg.id), Some(rg.clone())),
        (PlaylistItem::File(lists.join("missing").join("gone.flac")), None),
    ];
    if cfg!(unix) {
        expected.push((PlaylistItem::Library(a.id), Some(a.clone())));
    }
    for path in [&utf8, &utf16_path] {
        let id = fx.lib.import_m3u(path, "Mix").unwrap();
        let got: Vec<_> = entries(&fx, id).into_iter().map(|e| (e.item, e.track)).collect();
        assert_eq!(got, expected, "{}", path.display());
    }

    let revision = fx.lib.revision().unwrap();
    let err = fx.lib.import_m3u(&lists.join("nope.m3u"), "Nope").unwrap_err();
    assert!(
        matches!(&err, LibraryError::Io(io) if io.kind() == std::io::ErrorKind::NotFound),
        "{err}"
    );
    let err = fx.lib.import_m3u(&utf8, " ").unwrap_err();
    assert!(matches!(err, LibraryError::EmptyPlaylistName), "{err}");
    assert_eq!(
        fx.lib.revision().unwrap(),
        revision,
        "failed imports change nothing"
    );
    assert_eq!(fx.lib.playlists().unwrap().len(), 2);
}
