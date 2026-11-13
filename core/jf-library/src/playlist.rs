//! Playlists: ordered entries of library tracks, files and plugin items, with
//! M3U8 import and export.
//!
//! Positions run `0..n` without gaps in every playlist, guarded by the unique
//! (playlist, position) index. A change moves the positions it touches to
//! pending negative values (`-1 - new`) and then settles them all in one
//! statement, so no intermediate state ever clashes.

use std::io::{self, Read};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::item::{self, ITEM_COLUMN_COUNT, ITEM_COLUMNS, StoredItem};
use crate::lyrics::decode_text;
use crate::normalize::normalize;
use crate::query::{self, TRACK_COLUMNS, non_negative, to_i64};
use crate::scan::bump_revision;
use crate::util::{clean_path, unix_now_ms, write_atomic};
use crate::{Library, LibraryError, Playlist, PlaylistEntry, PlaylistItem};

/// Larger files are not playlists.
const MAX_M3U_BYTES: u64 = 16 << 20;

/// A plugin item's duration comes from `duration_ms` in its metadata, when that
/// is valid JSON holding a number there; CASE keeps `json_type` away from text
/// that is not JSON.
const ENTRY_DURATION: &str = "CASE WHEN x.kind = 2 AND json_valid(x.meta_json)
        THEN CASE WHEN json_type(x.meta_json, '$.duration_ms') IN ('integer', 'real')
            THEN json_extract(x.meta_json, '$.duration_ms') END
        ELSE t.duration_ms END";

fn playlist_select() -> String {
    format!(
        "SELECT p.id, p.name, p.created_at_ms, p.updated_at_ms,
            (SELECT COUNT(*) FROM playlist_items WHERE playlist_id = p.id),
            (SELECT CAST(IFNULL(SUM({ENTRY_DURATION}), 0) AS INTEGER)
                FROM playlist_items x LEFT JOIN tracks t ON t.path = x.path WHERE x.playlist_id = p.id),
            (SELECT t.artwork_hash FROM playlist_items x JOIN tracks t ON t.path = x.path
                WHERE x.playlist_id = p.id AND t.artwork_hash IS NOT NULL ORDER BY x.position LIMIT 1)
         FROM playlists p"
    )
}

fn playlist_from_row(row: &Row<'_>) -> rusqlite::Result<Playlist> {
    Ok(Playlist {
        id: row.get(0)?,
        name: row.get(1)?,
        created_at_ms: row.get(2)?,
        updated_at_ms: row.get(3)?,
        entry_count: row.get(4)?,
        duration_ms: non_negative(row.get(5)?),
        artwork_hash: row.get(6)?,
    })
}

/// Every playlist by name.
pub(crate) fn list(conn: &Connection) -> Result<Vec<Playlist>, LibraryError> {
    let sql = format!("{} ORDER BY p.name_norm, p.id", playlist_select());
    let rows = conn
        .prepare_cached(&sql)?
        .query_map([], playlist_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(crate) fn get(conn: &Connection, id: i64) -> Result<Option<Playlist>, LibraryError> {
    let sql = format!("{} WHERE p.id = ?1", playlist_select());
    Ok(conn
        .prepare_cached(&sql)?
        .query_row([id], playlist_from_row)
        .optional()?)
}

pub(crate) fn create(conn: &Connection, name: &str) -> Result<i64, LibraryError> {
    let tx = conn.unchecked_transaction()?;
    let id = insert_playlist(&tx, name)?;
    bump_revision(&tx)?;
    tx.commit()?;
    Ok(id)
}

pub(crate) fn rename(conn: &Connection, id: i64, name: &str) -> Result<(), LibraryError> {
    let name = valid_name(name)?;
    let tx = conn.unchecked_transaction()?;
    ensure_exists(&tx, id)?;
    let changed = tx.execute(
        "UPDATE playlists SET name = ?2, name_norm = ?3, updated_at_ms = ?4 WHERE id = ?1 AND name IS NOT ?2",
        params![id, name, normalize(name), unix_now_ms()],
    )?;
    if changed > 0 {
        bump_revision(&tx)?;
    }
    tx.commit()?;
    Ok(())
}

pub(crate) fn delete(conn: &Connection, id: i64) -> Result<(), LibraryError> {
    let tx = conn.unchecked_transaction()?;
    // Entries go with it (ON DELETE CASCADE).
    if tx.execute("DELETE FROM playlists WHERE id = ?1", [id])? == 0 {
        return Err(LibraryError::PlaylistNotFound(id));
    }
    bump_revision(&tx)?;
    tx.commit()?;
    Ok(())
}

/// A page of entries, from position `offset`.
pub(crate) fn entries(
    conn: &Connection,
    id: i64,
    offset: usize,
    limit: usize,
) -> Result<Vec<PlaylistEntry>, LibraryError> {
    let sql = format!(
        "SELECT x.id, x.position, {ITEM_COLUMNS}, {TRACK_COLUMNS}
         FROM playlist_items x LEFT JOIN tracks t ON t.path = x.path
         WHERE x.playlist_id = ?1 AND x.position >= ?2 ORDER BY x.position LIMIT ?3"
    );
    let rows = conn
        .prepare_cached(&sql)?
        .query_map(params![id, to_i64(offset), to_i64(limit)], |row| {
            let track = query::joined_track_at(row, 2 + ITEM_COLUMN_COUNT)?;
            Ok(PlaylistEntry {
                entry_id: row.get(0)?,
                position: usize::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                item: item::item_at(row, 2, track.as_ref())?,
                track,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(crate) fn add(
    conn: &Connection,
    id: i64,
    items: &[PlaylistItem],
    at: Option<usize>,
) -> Result<Vec<i64>, LibraryError> {
    let tx = conn.unchecked_transaction()?;
    let len = entry_count(&tx, id)?;
    let stored = items
        .iter()
        .map(|item| item::resolve(&tx, item))
        .collect::<Result<Vec<_>, _>>()?;
    let ids = insert_entries(&tx, id, len, &stored, at)?;
    if !ids.is_empty() {
        touch(&tx, id)?;
        bump_revision(&tx)?;
    }
    tx.commit()?;
    Ok(ids)
}

pub(crate) fn remove(conn: &Connection, id: i64, entry_ids: &[i64]) -> Result<(), LibraryError> {
    let tx = conn.unchecked_transaction()?;
    ensure_exists(&tx, id)?;
    let mut removed = 0;
    for entry_id in entry_ids {
        removed += tx
            .prepare_cached("DELETE FROM playlist_items WHERE id = ?1 AND playlist_id = ?2")?
            .execute(params![entry_id, id])?;
    }
    if removed > 0 {
        close_gaps(&tx, id)?;
        touch(&tx, id)?;
        bump_revision(&tx)?;
    }
    tx.commit()?;
    Ok(())
}

pub(crate) fn move_entry(conn: &Connection, id: i64, entry_id: i64, to: usize) -> Result<(), LibraryError> {
    let tx = conn.unchecked_transaction()?;
    let len = entry_count(&tx, id)?;
    let from: i64 = tx
        .query_row(
            "SELECT position FROM playlist_items WHERE id = ?1 AND playlist_id = ?2",
            params![entry_id, id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(LibraryError::PlaylistEntryNotFound {
            playlist_id: id,
            entry_id,
        })?;
    let to = to_i64(to).min(len - 1);
    if to != from {
        tx.execute(
            "UPDATE playlist_items SET position = -1 - ?2 WHERE id = ?1",
            params![entry_id, to],
        )?;
        if from < to {
            shift(&tx, id, from + 1, to, -1)?;
        } else {
            shift(&tx, id, to, from - 1, 1)?;
        }
        touch(&tx, id)?;
        bump_revision(&tx)?;
    }
    tx.commit()?;
    Ok(())
}

fn valid_name(name: &str) -> Result<&str, LibraryError> {
    let name = name.trim();
    if name.is_empty() {
        Err(LibraryError::EmptyPlaylistName)
    } else {
        Ok(name)
    }
}

fn insert_playlist(conn: &Connection, name: &str) -> Result<i64, LibraryError> {
    let name = valid_name(name)?;
    conn.prepare_cached(
        "INSERT INTO playlists (name, name_norm, created_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, ?3)",
    )?
    .execute(params![name, normalize(name), unix_now_ms()])?;
    Ok(conn.last_insert_rowid())
}

fn ensure_exists(conn: &Connection, id: i64) -> Result<(), LibraryError> {
    conn.prepare_cached("SELECT 1 FROM playlists WHERE id = ?1")?
        .query_row([id], |_| Ok(()))
        .optional()?
        .ok_or(LibraryError::PlaylistNotFound(id))
}

/// Number of entries; fails for a playlist that does not exist.
fn entry_count(conn: &Connection, id: i64) -> Result<i64, LibraryError> {
    ensure_exists(conn, id)?;
    Ok(conn
        .prepare_cached("SELECT COUNT(*) FROM playlist_items WHERE playlist_id = ?1")?
        .query_row([id], |row| row.get(0))?)
}

fn touch(conn: &Connection, id: i64) -> Result<(), LibraryError> {
    conn.prepare_cached("UPDATE playlists SET updated_at_ms = ?2 WHERE id = ?1")?
        .execute(params![id, unix_now_ms()])?;
    Ok(())
}

/// Inserts `items` at `at` (appended when `None` or past the end) into a
/// playlist of `len` entries; returns their entry ids in order.
fn insert_entries(
    conn: &Connection,
    playlist_id: i64,
    len: i64,
    items: &[StoredItem],
    at: Option<usize>,
) -> Result<Vec<i64>, LibraryError> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let at = at.map_or(len, |at| to_i64(at).min(len));
    if at < len {
        shift(conn, playlist_id, at, len - 1, to_i64(items.len()))?;
    }
    let now = unix_now_ms();
    let mut insert = conn.prepare_cached(
        "INSERT INTO playlist_items
            (playlist_id, position, kind, path, track_id, plugin, source_id, meta_json, added_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    let mut ids = Vec::with_capacity(items.len());
    for (offset, item) in (0_i64..).zip(items) {
        insert.execute(params![
            playlist_id,
            at + offset,
            item.kind,
            item.path,
            item.track_id,
            item.plugin,
            item.source_id,
            item.meta_json,
            now
        ])?;
        ids.push(conn.last_insert_rowid());
    }
    Ok(ids)
}

/// Moves the positions in `first..=last` by `delta`, settling every pending position.
fn shift(conn: &Connection, playlist_id: i64, first: i64, last: i64, delta: i64) -> Result<(), LibraryError> {
    conn.prepare_cached(
        "UPDATE playlist_items SET position = -1 - (position + ?4)
         WHERE playlist_id = ?1 AND position BETWEEN ?2 AND ?3",
    )?
    .execute(params![playlist_id, first, last, delta])?;
    settle(conn, playlist_id)
}

/// Renumbers the entries `0..n` in their current order, after removals.
fn close_gaps(conn: &Connection, playlist_id: i64) -> Result<(), LibraryError> {
    let rows = conn
        .prepare_cached("SELECT id, position FROM playlist_items WHERE playlist_id = ?1 ORDER BY position")?
        .query_map([playlist_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut pend = conn.prepare_cached("UPDATE playlist_items SET position = -1 - ?2 WHERE id = ?1")?;
    for (index, (entry_id, position)) in (0_i64..).zip(rows) {
        if position != index {
            pend.execute(params![entry_id, index])?;
        }
    }
    settle(conn, playlist_id)
}

/// Turns every pending position `-1 - p` into `p`.
fn settle(conn: &Connection, playlist_id: i64) -> Result<(), LibraryError> {
    conn.prepare_cached(
        "UPDATE playlist_items SET position = -1 - position WHERE playlist_id = ?1 AND position < 0",
    )?
    .execute([playlist_id])?;
    Ok(())
}

/// Reads an M3U or M3U8 playlist into a new playlist; see [`Library::import_m3u`].
pub(crate) fn import_m3u(lib: &Library, path: &Path, name: &str) -> Result<i64, LibraryError> {
    valid_name(name)?;
    let mut bytes = Vec::new();
    lib.fs
        .open(path)?
        .take(MAX_M3U_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_M3U_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "playlist file larger than 16 MiB").into());
    }
    let base = clean_path(path)?
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let files: Vec<PathBuf> = decode_text(&bytes)
        .lines()
        .filter_map(|line| m3u_file(&base, line))
        .collect();

    let tx = lib.conn.unchecked_transaction()?;
    let id = insert_playlist(&tx, name)?;
    let mut items = Vec::with_capacity(files.len());
    for file in &files {
        let key = item::file_key(file)?;
        let track_id: Option<i64> = tx
            .prepare_cached("SELECT id FROM tracks WHERE path = ?1")?
            .query_row([&key], |row| row.get(0))
            .optional()?;
        items.push(match track_id {
            Some(track_id) => StoredItem::library(key, track_id),
            None => StoredItem::file(key),
        });
    }
    insert_entries(&tx, id, 0, &items, None)?;
    bump_revision(&tx)?;
    tx.commit()?;
    Ok(id)
}

/// The file an M3U line names, made absolute against the playlist's folder.
/// `None` for blank lines, comments and directives, and URLs other than `file:`.
fn m3u_file(base: &Path, line: &str) -> Option<PathBuf> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    if let Some(scheme) = url_scheme(line) {
        if scheme.eq_ignore_ascii_case("file") {
            let path = file_url_path(&line[scheme.len() + 1..]);
            if path.is_none() {
                tracing::warn!(line, "skipping a file URL that names no local path");
            }
            return path;
        }
        tracing::warn!(line, "skipping a playlist entry that is not a local file");
        return None;
    }
    // Playlists written on Windows use backslashes.
    let native = if cfg!(windows) {
        line.to_owned()
    } else {
        line.replace('\\', "/")
    };
    clean_path(&base.join(native)).ok()
}

/// The scheme of `scheme://…`; a Windows drive (`C:\`) has no `//` and a
/// one-letter scheme, so it never looks like one.
fn url_scheme(line: &str) -> Option<&str> {
    let end = line.find("://")?;
    let scheme = &line[..end];
    let mut chars = scheme.chars();
    let valid = scheme.len() >= 2
        && chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then_some(scheme)
}

/// `//host/path` of a `file:` URL, percent-decoded; only an empty host or
/// `localhost` is local.
fn file_url_path(rest: &str) -> Option<PathBuf> {
    let rest = rest.strip_prefix("//")?;
    let slash = rest.find('/')?;
    let host = &rest[..slash];
    if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
        return None;
    }
    let decoded = percent_decode(&rest[slash..])?;
    // file:///C:/Music/x.flac names C:/Music/x.flac on Windows.
    let decoded = if cfg!(windows) && decoded.as_bytes().get(2) == Some(&b':') {
        decoded[1..].to_owned()
    } else {
        decoded
    };
    Some(PathBuf::from(decoded))
}

/// Decodes `%XX` escapes; `None` for a broken escape or bytes that are not UTF-8.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Writes a playlist as M3U8; see [`Library::export_m3u`].
pub(crate) fn export_m3u(lib: &Library, id: i64, path: &Path) -> Result<(), LibraryError> {
    ensure_exists(&lib.conn, id)?;
    let target = clean_path(path)?;
    let base = target.parent().map(Path::to_path_buf).unwrap_or_default();
    let rows = lib
        .conn
        .prepare_cached(
            "SELECT x.path, t.title, t.artist, t.duration_ms
             FROM playlist_items x LEFT JOIN tracks t ON t.path = x.path
             WHERE x.playlist_id = ?1 AND x.path IS NOT NULL ORDER BY x.position",
        )?
        .query_map([id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut out = String::from("#EXTM3U\n");
    for (file, title, artist, duration_ms) in rows {
        let file = PathBuf::from(file);
        let seconds = duration_ms.map_or(-1, |ms| (ms + 500) / 1000);
        let title = title.unwrap_or_else(|| {
            file.file_stem()
                .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
        });
        let label = match artist {
            Some(artist) => format!("{artist} - {title}"),
            None => title,
        };
        out.push_str(&format!(
            "#EXTINF:{seconds},{}\n",
            label.replace(['\r', '\n'], " ")
        ));
        out.push_str(&m3u_line(&base, &file));
        out.push('\n');
    }
    write_atomic(&target, out.as_bytes(), true)?;
    Ok(())
}

/// A path relative to the playlist's folder when the file is below it, else absolute.
fn m3u_line(base: &Path, file: &Path) -> String {
    match file.strip_prefix(base) {
        Ok(relative) if !relative.as_os_str().is_empty() => {
            let line = relative.to_string_lossy();
            // A leading '#' would read back as a comment.
            if line.starts_with('#') {
                format!(".{}{line}", std::path::MAIN_SEPARATOR)
            } else {
                line.into_owned()
            }
        }
        _ => file.to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m3u_lines_name_local_files() {
        let base = clean_path(Path::new("/lists")).unwrap();
        assert_eq!(m3u_file(&base, "  "), None);
        assert_eq!(m3u_file(&base, "#EXTINF:12,A - B"), None);
        assert_eq!(m3u_file(&base, "https://example.com/a.mp3"), None);
        assert_eq!(m3u_file(&base, "a/b.flac"), Some(base.join("a").join("b.flac")));
        assert_eq!(
            m3u_file(&base, "../music/b.flac"),
            clean_path(Path::new("/music/b.flac")).ok()
        );
        assert_eq!(url_scheme("C:\\Music\\x.flac"), None);
        assert_eq!(url_scheme("C://x"), None);
        assert_eq!(url_scheme("file:///x"), Some("file"));
        assert_eq!(url_scheme("rtsp+tcp://x"), Some("rtsp+tcp"));
    }

    #[test]
    #[cfg(unix)]
    fn file_urls_are_decoded() {
        let base = Path::new("/lists");
        assert_eq!(
            m3u_file(base, "file:///music/%E6%99%B4%20sky.flac"),
            Some(PathBuf::from("/music/晴 sky.flac"))
        );
        assert_eq!(
            m3u_file(base, "FILE://localhost/music/a.flac"),
            Some(PathBuf::from("/music/a.flac"))
        );
        assert_eq!(m3u_file(base, "file://server/music/a.flac"), None);
        assert_eq!(m3u_file(base, "file:///music/%zz.flac"), None);
        assert_eq!(m3u_file(base, "file:///music/%ff.flac"), None, "not UTF-8");
        assert_eq!(
            m3u_file(base, "sub\\dir\\a.flac"),
            Some(PathBuf::from("/lists/sub/dir/a.flac"))
        );
    }

    #[test]
    #[cfg(unix)]
    fn exported_lines_are_relative_below_the_playlist() {
        let base = Path::new("/music");
        assert_eq!(m3u_line(base, Path::new("/music/a/b.flac")), "a/b.flac");
        assert_eq!(m3u_line(base, Path::new("/music/#1.flac")), "./#1.flac");
        assert_eq!(m3u_line(base, Path::new("/other/c.flac")), "/other/c.flac");
        assert_eq!(m3u_line(base, Path::new("/musical/c.flac")), "/musical/c.flac");
    }
}
