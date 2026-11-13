//! Database schema and migrations.
//!
//! `PRAGMA user_version` holds the number of migrations applied. Each migration
//! runs in its own transaction together with the version bump, so a crash leaves
//! the database at the previous version, never half-migrated.
//!
//! Schema version 1:
//!
//! | Table | Holds |
//! | --- | --- |
//! | `meta` | key → integer; `revision` bumps on every change |
//! | `roots` | folders the scanner walks |
//! | `tracks` | one row per audio file: file identity (path, size, mtime), tags, audio properties, NFKC-normalised search columns |
//! | `albums` | grouped by normalised (album title, album artist or else track artist) |
//! | `artists` | unique by normalised name |
//! | `track_artists` | track → artists after splitting the artist tag |
//! | `artwork` | blake3 hash → file in `<cache_dir>/artwork` |
//! | `*_fts`, `*_tri` | FTS5 external-content indexes over the normalised columns of tracks, albums and artists: `unicode61` for words and prefixes, `trigram` for substrings |
//!
//! Triggers keep the FTS indexes in step with their content tables.
//!
//! Schema version 2 adds, without changing any version 1 table:
//!
//! | Table | Holds |
//! | --- | --- |
//! | `playlists` | name, creation and change times (ms) |
//! | `playlist_items` | ordered entries, positions `0..n` per playlist: a library track (its path and last id), a file (its path) or a plugin item (plugin, id, metadata JSON) |
//! | `item_stats` | favourite time, play and skip counts, last play (ms), keyed by path for tracks and files and by (plugin, id) for plugin items |
//!
//! and indexes on `tracks` for recently added tracks (`tracks_added`) and for
//! sorting albums by year and by first scan (`tracks_album_year`,
//! `tracks_album_added`: one index seek per album). Playlist entries and
//! statistics refer to tracks by path, never by foreign key: removing a track
//! keeps them, and they find the track again when its file is scanned back in.
//! Playlist and entry ids are `AUTOINCREMENT`, so a UI holding a stale id never
//! hits a newer playlist or entry.

use rusqlite::Connection;

use crate::LibraryError;

const MIGRATIONS: &[&str] = &[V1, V2];

/// The schema version this build migrates to.
pub(crate) const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// Brings the database up to `target`, at most [`SCHEMA_VERSION`] (the app
/// always asks for that); a database already past `target` is left alone.
pub(crate) fn migrate_to(conn: &Connection, target: i64) -> Result<(), LibraryError> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if current > SCHEMA_VERSION {
        return Err(LibraryError::SchemaTooNew {
            found: current,
            supported: SCHEMA_VERSION,
        });
    }
    let target = target.clamp(0, SCHEMA_VERSION) as usize;
    for (index, sql) in MIGRATIONS
        .iter()
        .enumerate()
        .take(target)
        .skip(current.max(0) as usize)
    {
        let version = index as i64 + 1;
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
        tracing::info!(version, "library schema migrated");
    }
    Ok(())
}

const V1: &str = r#"
CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value INTEGER NOT NULL
) WITHOUT ROWID;
INSERT INTO meta (key, value) VALUES ('revision', 0);

CREATE TABLE roots (
    id       INTEGER PRIMARY KEY,
    path     TEXT NOT NULL UNIQUE,
    added_at INTEGER NOT NULL
);

CREATE TABLE artwork (
    hash      TEXT PRIMARY KEY,
    file_name TEXT NOT NULL,
    mime      TEXT,
    byte_len  INTEGER NOT NULL
) WITHOUT ROWID;

CREATE TABLE artists (
    id        INTEGER PRIMARY KEY,
    name      TEXT NOT NULL,
    name_norm TEXT NOT NULL UNIQUE
);

CREATE TABLE albums (
    id           INTEGER PRIMARY KEY,
    title        TEXT NOT NULL,
    album_artist TEXT,
    title_norm   TEXT NOT NULL,
    artist_norm  TEXT NOT NULL,
    UNIQUE (title_norm, artist_norm)
);

CREATE TABLE tracks (
    id                  INTEGER PRIMARY KEY,
    root_id             INTEGER NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
    path                TEXT NOT NULL UNIQUE,
    file_size           INTEGER NOT NULL,
    mtime_ns            INTEGER NOT NULL,
    title               TEXT NOT NULL,
    artist              TEXT,
    album               TEXT,
    album_artist        TEXT,
    album_id            INTEGER REFERENCES albums(id),
    track_no            INTEGER,
    disc_no             INTEGER,
    year                INTEGER,
    genre               TEXT,
    duration_ms         INTEGER,
    sample_rate         INTEGER,
    channels            INTEGER,
    bit_depth           INTEGER,
    codec               TEXT,
    rg_track_gain       REAL,
    rg_track_peak       REAL,
    rg_album_gain       REAL,
    rg_album_peak       REAL,
    isrc                TEXT,
    has_embedded_lyrics INTEGER NOT NULL DEFAULT 0,
    artwork_hash        TEXT REFERENCES artwork(hash),
    title_norm          TEXT NOT NULL,
    artist_norm         TEXT NOT NULL,
    album_norm          TEXT NOT NULL,
    added_at            INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL
);
CREATE INDEX tracks_root ON tracks (root_id);
CREATE INDEX tracks_album ON tracks (album_id, disc_no, track_no);
CREATE INDEX tracks_title ON tracks (title_norm);
CREATE INDEX tracks_artwork ON tracks (artwork_hash);

CREATE TABLE track_artists (
    track_id  INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    artist_id INTEGER NOT NULL REFERENCES artists(id),
    position  INTEGER NOT NULL,
    PRIMARY KEY (track_id, artist_id)
) WITHOUT ROWID;
CREATE INDEX track_artists_artist ON track_artists (artist_id);

CREATE VIRTUAL TABLE tracks_fts USING fts5(
    title_norm, artist_norm, album_norm,
    content = 'tracks', content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);
CREATE VIRTUAL TABLE tracks_tri USING fts5(
    title_norm, artist_norm, album_norm,
    content = 'tracks', content_rowid = 'id',
    tokenize = 'trigram'
);
CREATE TRIGGER tracks_fts_insert AFTER INSERT ON tracks BEGIN
    INSERT INTO tracks_fts (rowid, title_norm, artist_norm, album_norm)
        VALUES (new.id, new.title_norm, new.artist_norm, new.album_norm);
    INSERT INTO tracks_tri (rowid, title_norm, artist_norm, album_norm)
        VALUES (new.id, new.title_norm, new.artist_norm, new.album_norm);
END;
CREATE TRIGGER tracks_fts_delete AFTER DELETE ON tracks BEGIN
    INSERT INTO tracks_fts (tracks_fts, rowid, title_norm, artist_norm, album_norm)
        VALUES ('delete', old.id, old.title_norm, old.artist_norm, old.album_norm);
    INSERT INTO tracks_tri (tracks_tri, rowid, title_norm, artist_norm, album_norm)
        VALUES ('delete', old.id, old.title_norm, old.artist_norm, old.album_norm);
END;
CREATE TRIGGER tracks_fts_update AFTER UPDATE OF title_norm, artist_norm, album_norm ON tracks BEGIN
    INSERT INTO tracks_fts (tracks_fts, rowid, title_norm, artist_norm, album_norm)
        VALUES ('delete', old.id, old.title_norm, old.artist_norm, old.album_norm);
    INSERT INTO tracks_tri (tracks_tri, rowid, title_norm, artist_norm, album_norm)
        VALUES ('delete', old.id, old.title_norm, old.artist_norm, old.album_norm);
    INSERT INTO tracks_fts (rowid, title_norm, artist_norm, album_norm)
        VALUES (new.id, new.title_norm, new.artist_norm, new.album_norm);
    INSERT INTO tracks_tri (rowid, title_norm, artist_norm, album_norm)
        VALUES (new.id, new.title_norm, new.artist_norm, new.album_norm);
END;

CREATE VIRTUAL TABLE albums_fts USING fts5(
    title_norm, artist_norm,
    content = 'albums', content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);
CREATE VIRTUAL TABLE albums_tri USING fts5(
    title_norm, artist_norm,
    content = 'albums', content_rowid = 'id',
    tokenize = 'trigram'
);
CREATE TRIGGER albums_fts_insert AFTER INSERT ON albums BEGIN
    INSERT INTO albums_fts (rowid, title_norm, artist_norm) VALUES (new.id, new.title_norm, new.artist_norm);
    INSERT INTO albums_tri (rowid, title_norm, artist_norm) VALUES (new.id, new.title_norm, new.artist_norm);
END;
CREATE TRIGGER albums_fts_delete AFTER DELETE ON albums BEGIN
    INSERT INTO albums_fts (albums_fts, rowid, title_norm, artist_norm)
        VALUES ('delete', old.id, old.title_norm, old.artist_norm);
    INSERT INTO albums_tri (albums_tri, rowid, title_norm, artist_norm)
        VALUES ('delete', old.id, old.title_norm, old.artist_norm);
END;
CREATE TRIGGER albums_fts_update AFTER UPDATE OF title_norm, artist_norm ON albums BEGIN
    INSERT INTO albums_fts (albums_fts, rowid, title_norm, artist_norm)
        VALUES ('delete', old.id, old.title_norm, old.artist_norm);
    INSERT INTO albums_tri (albums_tri, rowid, title_norm, artist_norm)
        VALUES ('delete', old.id, old.title_norm, old.artist_norm);
    INSERT INTO albums_fts (rowid, title_norm, artist_norm) VALUES (new.id, new.title_norm, new.artist_norm);
    INSERT INTO albums_tri (rowid, title_norm, artist_norm) VALUES (new.id, new.title_norm, new.artist_norm);
END;

CREATE VIRTUAL TABLE artists_fts USING fts5(
    name_norm,
    content = 'artists', content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);
CREATE VIRTUAL TABLE artists_tri USING fts5(
    name_norm,
    content = 'artists', content_rowid = 'id',
    tokenize = 'trigram'
);
CREATE TRIGGER artists_fts_insert AFTER INSERT ON artists BEGIN
    INSERT INTO artists_fts (rowid, name_norm) VALUES (new.id, new.name_norm);
    INSERT INTO artists_tri (rowid, name_norm) VALUES (new.id, new.name_norm);
END;
CREATE TRIGGER artists_fts_delete AFTER DELETE ON artists BEGIN
    INSERT INTO artists_fts (artists_fts, rowid, name_norm) VALUES ('delete', old.id, old.name_norm);
    INSERT INTO artists_tri (artists_tri, rowid, name_norm) VALUES ('delete', old.id, old.name_norm);
END;
CREATE TRIGGER artists_fts_update AFTER UPDATE OF name_norm ON artists BEGIN
    INSERT INTO artists_fts (artists_fts, rowid, name_norm) VALUES ('delete', old.id, old.name_norm);
    INSERT INTO artists_tri (artists_tri, rowid, name_norm) VALUES ('delete', old.id, old.name_norm);
    INSERT INTO artists_fts (rowid, name_norm) VALUES (new.id, new.name_norm);
    INSERT INTO artists_tri (rowid, name_norm) VALUES (new.id, new.name_norm);
END;
"#;

const V2: &str = r#"
CREATE TABLE playlists (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL,
    name_norm     TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
CREATE INDEX playlists_name ON playlists (name_norm);

-- kind: 0 library track, 1 file, 2 plugin item.
CREATE TABLE playlist_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    kind        INTEGER NOT NULL,
    path        TEXT,
    track_id    INTEGER,
    plugin      TEXT,
    source_id   TEXT,
    meta_json   TEXT,
    added_at_ms INTEGER NOT NULL,
    CHECK ((kind = 0 AND path IS NOT NULL AND track_id IS NOT NULL)
        OR (kind = 1 AND path IS NOT NULL)
        OR (kind = 2 AND path IS NULL AND plugin IS NOT NULL AND source_id IS NOT NULL))
);
CREATE UNIQUE INDEX playlist_items_position ON playlist_items (playlist_id, position);

-- One row per item key; kind is how the item was last referred to.
CREATE TABLE item_stats (
    id             INTEGER PRIMARY KEY,
    kind           INTEGER NOT NULL,
    path           TEXT,
    track_id       INTEGER,
    plugin         TEXT,
    source_id      TEXT,
    meta_json      TEXT,
    favorite_at_ms INTEGER,
    play_count     INTEGER NOT NULL DEFAULT 0,
    skip_count     INTEGER NOT NULL DEFAULT 0,
    last_played_ms INTEGER,
    CHECK ((kind IN (0, 1) AND path IS NOT NULL AND plugin IS NULL)
        OR (kind = 2 AND path IS NULL AND plugin IS NOT NULL AND source_id IS NOT NULL))
);
CREATE UNIQUE INDEX item_stats_path ON item_stats (path) WHERE path IS NOT NULL;
CREATE UNIQUE INDEX item_stats_source ON item_stats (plugin, source_id) WHERE plugin IS NOT NULL;
CREATE INDEX item_stats_favorites ON item_stats (favorite_at_ms) WHERE favorite_at_ms IS NOT NULL;
CREATE INDEX item_stats_recent ON item_stats (last_played_ms) WHERE last_played_ms IS NOT NULL;
CREATE INDEX item_stats_most ON item_stats (play_count, last_played_ms) WHERE play_count > 0;

CREATE INDEX tracks_added ON tracks (added_at, id);
CREATE INDEX tracks_album_year ON tracks (album_id, year);
CREATE INDEX tracks_album_added ON tracks (album_id, added_at);
"#;
