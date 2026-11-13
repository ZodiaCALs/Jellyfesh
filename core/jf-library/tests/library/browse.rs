//! Browse queries: an artist's albums, an album's tracks, sorted album pages.

use std::time::{Duration, Instant};

use jf_library::{Album, AlbumSort, Library, PlaylistItem};
use lofty::tag::{Accessor, ItemKey};
use rusqlite::{Connection, params};

use crate::fixtures::{self, Fixture};

/// One generated track and its tags.
#[derive(Default)]
struct Spec {
    file: &'static str,
    title: &'static str,
    artist: Option<&'static str>,
    album: &'static str,
    album_artist: Option<&'static str>,
    year: Option<&'static str>,
    disc: Option<u32>,
    track: Option<u32>,
    cover: bool,
}

fn add(fx: &Fixture, spec: Spec) {
    let path = fx.path(spec.file);
    fixtures::wav(&path, 50);
    fixtures::tag(&path, |t| {
        t.set_title(spec.title.to_owned());
        t.set_album(spec.album.to_owned());
        if let Some(artist) = spec.artist {
            t.set_artist(artist.to_owned());
        }
        if let Some(album_artist) = spec.album_artist {
            fixtures::set_text(t, ItemKey::AlbumArtist, album_artist);
        }
        if let Some(year) = spec.year {
            fixtures::set_text(t, ItemKey::RecordingDate, year);
        }
        if let Some(disc) = spec.disc {
            t.set_disk(disc);
        }
        if let Some(track) = spec.track {
            t.set_track(track);
        }
        if spec.cover {
            t.push_picture(fixtures::front_cover(fixtures::png(4, 4, [1, 2, 3])));
        }
    });
}

/// Nova's discography, an album Nova is featured on, others' albums and an
/// album without any artist.
fn discography() -> Fixture {
    let fx = Fixture::new();
    let nova = Some("Nova");
    let dawn = |file, title, disc, track| Spec {
        file,
        title,
        artist: nova,
        album: "Dawn",
        year: Some("2003"),
        disc,
        track,
        ..Spec::default()
    };
    for spec in [
        dawn("dawn/1.wav", "Second Disc Opener", Some(2), Some(1)),
        dawn("dawn/2.wav", "B Side", Some(1), Some(2)),
        dawn("dawn/3.wav", "Opener", Some(1), Some(1)),
        dawn("dawn/4.wav", "Third", None, Some(3)),
        dawn("dawn/5.wav", "Hidden", Some(1), None),
        dawn("dawn/6.wav", "Alpha Bonus", None, None),
        Spec {
            file: "zenith/1.wav",
            title: "Peak",
            artist: nova,
            album: "Zenith",
            year: Some("1999"),
            cover: true,
            ..Spec::default()
        },
        Spec {
            file: "echoes/1.wav",
            title: "Echo",
            artist: nova,
            album: "Echoes",
            ..Spec::default()
        },
        Spec {
            file: "various/1.wav",
            title: "Guest Spot",
            artist: Some("Other feat. Nova"),
            album: "Various Hits",
            album_artist: Some("Various Artists"),
            year: Some("2001"),
            ..Spec::default()
        },
        Spec {
            file: "remixed/1.wav",
            title: "Remix",
            artist: Some("DJ X"),
            album: "Remixed",
            album_artist: nova,
            year: Some("2010"),
            ..Spec::default()
        },
        Spec {
            file: "solo/1.wav",
            title: "Alone",
            artist: Some("Other"),
            album: "Solo",
            year: Some("2005"),
            ..Spec::default()
        },
        Spec {
            file: "nameless/1.wav",
            title: "Anonymous",
            album: "Nameless",
            year: Some("2000"),
            ..Spec::default()
        },
    ] {
        add(&fx, spec);
    }
    assert_eq!(fx.scan().added, 12);
    fx
}

fn titles(albums: &[Album]) -> Vec<&str> {
    albums.iter().map(|a| a.title.as_str()).collect()
}

fn sorted(fx: &Fixture, sort: AlbumSort, descending: bool) -> Vec<String> {
    fx.lib
        .albums_sorted(sort, descending, 0, 100)
        .unwrap()
        .into_iter()
        .map(|a| a.title)
        .collect()
}

#[test]
fn an_artists_albums_come_by_year_then_title() {
    let fx = discography();
    let nova = fx
        .lib
        .artists(0, 100)
        .unwrap()
        .into_iter()
        .find(|a| a.name == "Nova")
        .unwrap();
    let albums = fx.lib.artist_albums(nova.id).unwrap();
    assert_eq!(
        titles(&albums),
        ["Zenith", "Various Hits", "Dawn", "Remixed", "Echoes"],
        "featured on Various Hits, album artist of Remixed; no year last"
    );
    let zenith = &albums[0];
    assert_eq!(zenith.year, Some(1999));
    assert!(zenith.artwork_hash.is_some());
    assert_eq!(fx.lib.album(zenith.id).unwrap().as_ref(), Some(zenith));
    assert_eq!(albums[2].track_count, 6);
    assert!(fx.lib.artist_albums(-1).unwrap().is_empty());
}

#[test]
fn an_albums_tracks_come_by_disc_track_and_title() {
    let fx = discography();
    let dawn = fx
        .lib
        .albums(0, 100)
        .unwrap()
        .into_iter()
        .find(|a| a.title == "Dawn")
        .unwrap();
    let tracks: Vec<String> = fx
        .lib
        .album_tracks(dawn.id)
        .unwrap()
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(
        tracks,
        [
            "Opener",
            "B Side",
            "Third",
            "Alpha Bonus",
            "Hidden",
            "Second Disc Opener"
        ],
        "no disc counts as disc 1; no track number goes last on its disc"
    );
    assert!(fx.lib.album_tracks(-1).unwrap().is_empty());
}

#[test]
fn album_pages_sort_by_title_artist_year_and_added() {
    let fx = discography();
    assert_eq!(
        sorted(&fx, AlbumSort::Title, false),
        [
            "Dawn",
            "Echoes",
            "Nameless",
            "Remixed",
            "Solo",
            "Various Hits",
            "Zenith"
        ]
    );
    assert_eq!(
        sorted(&fx, AlbumSort::Title, true),
        [
            "Zenith",
            "Various Hits",
            "Solo",
            "Remixed",
            "Nameless",
            "Echoes",
            "Dawn"
        ]
    );
    assert_eq!(
        fx.lib.albums(0, 100).unwrap(),
        fx.lib.albums_sorted(AlbumSort::Title, false, 0, 100).unwrap()
    );
    assert_eq!(
        sorted(&fx, AlbumSort::Artist, false),
        [
            "Zenith",
            "Dawn",
            "Remixed",
            "Echoes",
            "Solo",
            "Various Hits",
            "Nameless"
        ],
        "artist, then year; unknown artist and year last"
    );
    assert_eq!(
        sorted(&fx, AlbumSort::Artist, true),
        [
            "Various Hits",
            "Solo",
            "Remixed",
            "Dawn",
            "Zenith",
            "Echoes",
            "Nameless"
        ]
    );
    assert_eq!(
        sorted(&fx, AlbumSort::Year, false),
        [
            "Zenith",
            "Nameless",
            "Various Hits",
            "Dawn",
            "Solo",
            "Remixed",
            "Echoes"
        ]
    );
    assert_eq!(
        sorted(&fx, AlbumSort::Year, true),
        [
            "Remixed",
            "Solo",
            "Dawn",
            "Various Hits",
            "Nameless",
            "Zenith",
            "Echoes"
        ]
    );

    // Added: Zenith's tracks were first seen long ago, Dawn's in the future.
    let conn = rusqlite::Connection::open(fx.dir.path().join("db").join("library.db")).unwrap();
    for (album, added_at) in [("Zenith", 1_i64), ("Dawn", 4_000_000_000)] {
        conn.execute(
            "UPDATE tracks SET added_at = ?2 WHERE album = ?1",
            rusqlite::params![album, added_at],
        )
        .unwrap();
    }
    let added = sorted(&fx, AlbumSort::Added, false);
    assert_eq!(added.first().map(String::as_str), Some("Zenith"));
    assert_eq!(added.last().map(String::as_str), Some("Dawn"));
    let newest = sorted(&fx, AlbumSort::Added, true);
    assert_eq!(newest.first().map(String::as_str), Some("Dawn"));
    assert_eq!(newest.last().map(String::as_str), Some("Zenith"));

    let page = fx.lib.albums_sorted(AlbumSort::Year, false, 2, 3).unwrap();
    assert_eq!(titles(&page), ["Various Hits", "Dawn", "Solo"]);
    let zenith = fx.lib.albums_sorted(AlbumSort::Year, false, 0, 1).unwrap();
    assert!(
        zenith[0].artwork_hash.is_some(),
        "every album list carries the artwork"
    );
}

/// Album pages, an artist's albums and a 10 000-entry playlist on 50 000
/// synthetic tracks in 5 000 albums.
#[test]
#[ignore = "timing check on 50k synthetic tracks; run with --ignored --nocapture"]
fn browsing_stays_fast_on_50k_tracks() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("library.db");
    let lib = Library::open(&db, &dir.path().join("cache")).unwrap();
    let root = lib.add_root(dir.path()).unwrap();
    drop(lib);

    let mut conn = Connection::open(&db).unwrap();
    let tx = conn.transaction().unwrap();
    for album in 0..5_000 {
        let artist = format!("artist {}", album % 500);
        tx.execute(
            "INSERT INTO albums (title, album_artist, title_norm, artist_norm) VALUES (?1, ?2, ?1, ?2)",
            params![format!("album {album}"), artist],
        )
        .unwrap();
    }
    for artist in 0..500 {
        let name = format!("artist {artist}");
        tx.execute("INSERT INTO artists (name, name_norm) VALUES (?1, ?1)", [name])
            .unwrap();
    }
    for i in 0..50_000_i64 {
        let album = i % 5_000;
        tx.execute(
            "INSERT INTO tracks (root_id, path, file_size, mtime_ns, title, artist, album, album_id,
                track_no, year, duration_ms, title_norm, artist_norm, album_norm, added_at, updated_at)
             VALUES (?1, ?2, 1, 1, ?3, ?4, ?5, ?6, ?7, ?8, 200000, ?3, ?4, ?5, ?9, ?9)",
            params![
                root,
                format!("/m/{i}.flac"),
                format!("track {i}"),
                format!("artist {}", album % 500),
                format!("album {album}"),
                album + 1,
                i / 5_000 + 1,
                1950 + album % 70,
                i % 997,
            ],
        )
        .unwrap();
        tx.execute(
            "INSERT INTO track_artists (track_id, artist_id, position) VALUES (?1, ?2, 0)",
            params![i + 1, album % 500 + 1],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    drop(conn);

    let lib = Library::open(&db, &dir.path().join("cache")).unwrap();
    let timed = |what: &str, run: &mut dyn FnMut() -> usize| {
        let started = Instant::now();
        let rows = run();
        let took = started.elapsed();
        println!("{what:>34}: {rows:>5} rows in {took:?}");
        assert!(took < Duration::from_millis(500), "{what}: {took:?}");
    };
    for sort in [
        AlbumSort::Title,
        AlbumSort::Artist,
        AlbumSort::Year,
        AlbumSort::Added,
    ] {
        for offset in [0, 4_900] {
            timed(&format!("albums by {sort:?} from {offset}"), &mut || {
                lib.albums_sorted(sort, true, offset, 100).unwrap().len()
            });
        }
    }
    timed("an artist's albums", &mut || lib.artist_albums(17).unwrap().len());
    timed("an album's tracks", &mut || lib.album_tracks(17).unwrap().len());
    timed("recently added", &mut || lib.recently_added(100).unwrap().len());

    let id = lib.create_playlist("Big").unwrap();
    let items: Vec<PlaylistItem> = (1..=10_000).map(PlaylistItem::Library).collect();
    timed("add 10 000 entries", &mut || {
        lib.add_to_playlist(id, &items, None).unwrap().len()
    });
    timed("insert one at the front", &mut || {
        lib.add_to_playlist(id, &items[..1], Some(0)).unwrap().len()
    });
    let entries = lib.playlist_entries(id, 0, 10_001).unwrap();
    timed("move the first entry to the end", &mut || {
        lib.move_playlist_entry(id, entries[0].entry_id, 10_000).unwrap();
        1
    });
    let every_tenth: Vec<i64> = entries.iter().step_by(10).map(|e| e.entry_id).collect();
    timed("remove every tenth entry", &mut || {
        lib.remove_from_playlist(id, &every_tenth).unwrap();
        every_tenth.len()
    });
    timed("a page deep in the playlist", &mut || {
        lib.playlist_entries(id, 8_000, 100).unwrap().len()
    });
    timed("list playlists with durations", &mut || {
        lib.playlists().unwrap().len()
    });
}
