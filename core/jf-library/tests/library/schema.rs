use jf_library::{Library, LibraryError, PlaylistItem, TrackQuery, TrackSort};
use rusqlite::Connection;

use crate::fixtures::{Fixture, standard};

const V1_TABLES: &[&str] = &[
    "meta",
    "roots",
    "tracks",
    "albums",
    "artists",
    "track_artists",
    "artwork",
    "tracks_fts",
    "tracks_tri",
    "albums_fts",
    "albums_tri",
    "artists_fts",
    "artists_tri",
];

const V2_OBJECTS: &[&str] = &[
    "playlists",
    "playlist_items",
    "item_stats",
    "tracks_added",
    "tracks_album_year",
    "tracks_album_added",
];

fn user_version(conn: &Connection) -> i64 {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
}

fn has_object(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_schema WHERE name = ?1",
        [name],
        |r| r.get::<_, i64>(0),
    )
    .unwrap()
        == 1
}

#[test]
fn a_fresh_database_is_created_and_migrated() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("nested").join("library.db");
    let lib = Library::open(&db, &dir.path().join("cache")).unwrap();
    assert_eq!(lib.revision().unwrap(), 0);
    assert_eq!(lib.stats().unwrap(), Default::default());
    drop(lib);

    let conn = Connection::open(&db).unwrap();
    assert_eq!(user_version(&conn), 2);
    let mode: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
    assert_eq!(mode, "wal");
    for name in V1_TABLES.iter().chain(V2_OBJECTS) {
        assert!(has_object(&conn, name), "missing {name}");
    }
    drop(conn);

    // Reopening an up-to-date database is a no-op and keeps the data.
    let lib = Library::open(&db, &dir.path().join("cache")).unwrap();
    lib.add_root(dir.path()).unwrap();
    drop(lib);
    let lib = Library::open(&db, &dir.path().join("cache")).unwrap();
    assert_eq!(lib.roots().unwrap(), vec![dir.path().to_path_buf()]);
}

#[test]
fn a_newer_schema_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("library.db");
    Connection::open(&db)
        .unwrap()
        .execute_batch("PRAGMA user_version = 99")
        .unwrap();
    let err = Library::open(&db, dir.path()).unwrap_err();
    assert!(
        matches!(
            err,
            LibraryError::SchemaTooNew {
                found: 99,
                supported: 2
            }
        ),
        "{err}"
    );
}

/// A library made and scanned with schema version 1, as v0.0.2 left it.
fn v1_library() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    std::fs::create_dir_all(&music).unwrap();
    let db = dir.path().join("db").join("library.db");
    let lib = Library::open_at_schema_version(&db, &dir.path().join("cache"), 1).unwrap();
    lib.add_root(&music).unwrap();
    let fx = Fixture { dir, music, lib };
    standard::build(&fx);
    assert_eq!(fx.scan().added, standard::GOOD_FILES);
    fx
}

#[test]
fn a_version_1_database_is_migrated_to_version_2_with_its_data() {
    let fx = v1_library();
    let db = fx.dir.path().join("db").join("library.db");
    let conn = Connection::open(&db).unwrap();
    assert_eq!(user_version(&conn), 1);
    for name in V1_TABLES {
        assert!(has_object(&conn, name), "v1 lacks {name}");
    }
    for name in V2_OBJECTS {
        assert!(!has_object(&conn, name), "v1 already has {name}");
    }
    drop(conn);

    let tracks = fx.all_tracks();
    let albums = fx.lib.albums(0, 100).unwrap();
    let artists = fx.lib.artists(0, 100).unwrap();
    let stats = fx.lib.stats().unwrap();
    let revision = fx.lib.revision().unwrap();
    let found = fx.lib.search("moon", 10).unwrap();
    let Fixture { dir, music, lib } = fx;
    drop(lib);

    let lib = Library::open(&db, &dir.path().join("cache")).unwrap();
    let fx = Fixture { dir, music, lib };
    let conn = Connection::open(&db).unwrap();
    assert_eq!(user_version(&conn), 2);
    for name in V1_TABLES.iter().chain(V2_OBJECTS) {
        assert!(has_object(&conn, name), "missing {name}");
    }
    drop(conn);

    assert_eq!(fx.all_tracks(), tracks);
    assert_eq!(fx.lib.albums(0, 100).unwrap(), albums);
    assert_eq!(fx.lib.artists(0, 100).unwrap(), artists);
    assert_eq!(fx.lib.stats().unwrap(), stats);
    assert_eq!(fx.lib.revision().unwrap(), revision, "migrating is not a change");
    assert_eq!(fx.lib.search("moon", 10).unwrap(), found);
    fx.assert_search_index_consistent();

    // The old tracks work with the new tables, and a rescan finds nothing to do.
    let id = fx.lib.create_playlist("Migrated").unwrap();
    let first = fx.track_at(standard::COVER_A);
    fx.lib
        .add_to_playlist(id, &[PlaylistItem::Library(first.id)], None)
        .unwrap();
    fx.lib
        .record_play(&PlaylistItem::Library(first.id), 1_000, true)
        .unwrap();
    assert_eq!(fx.lib.playlist_entries(id, 0, 10).unwrap()[0].track, Some(first));
    assert_eq!(fx.lib.most_played(10).unwrap().len(), 1);
    let report = fx.scan();
    assert_eq!(report.unchanged, standard::GOOD_FILES, "{report:?}");
    let newest = fx.lib.recently_added(1).unwrap();
    assert_eq!(newest.len(), 1);
    assert_eq!(
        fx.lib
            .tracks(&TrackQuery {
                sort: TrackSort::Added,
                descending: true,
                limit: 1,
                ..TrackQuery::default()
            })
            .unwrap(),
        newest
    );
}

#[test]
fn a_failed_migration_leaves_version_1_intact() {
    let fx = v1_library();
    let db = fx.dir.path().join("db").join("library.db");
    let cache = fx.dir.path().join("cache");
    let tracks = fx.all_tracks();
    drop(fx.lib);

    // Something in the way of the second half of the migration.
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch("CREATE TABLE item_stats (x INTEGER)").unwrap();
    drop(conn);
    let err = Library::open(&db, &cache).unwrap_err();
    assert!(matches!(err, LibraryError::Database(_)), "{err}");

    let conn = Connection::open(&db).unwrap();
    assert_eq!(user_version(&conn), 1);
    assert!(!has_object(&conn, "playlists"), "rolled back as a whole");
    assert!(!has_object(&conn, "tracks_added"));
    conn.execute_batch("DROP TABLE item_stats").unwrap();
    drop(conn);

    let lib = Library::open(&db, &cache).unwrap();
    let after = lib
        .tracks(&TrackQuery {
            limit: 1000,
            sort: TrackSort::Path,
            ..TrackQuery::default()
        })
        .unwrap();
    assert_eq!(after, tracks);
    assert!(lib.playlists().unwrap().is_empty());
}
