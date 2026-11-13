//! The incremental scanner.
//!
//! [`scan`] makes one pass over all roots:
//!
//! 1. Load `(path → id, root, size, mtime)` for every known track.
//! 2. Walk each root. A file whose size and mtime are unchanged is skipped; any
//!    other audio file is parsed (outside any transaction) and queued.
//! 3. Queued changes are written in transactions of [`BATCH_SIZE`].
//! 4. Known tracks that no walk produced are removed, in batches, unless their
//!    root's walk reported errors: an unmounted drive or an unreadable folder must
//!    never empty the library. They are removed by the next clean scan.
//! 5. Albums, artists and artwork that lost their last track are deleted.
//!
//! [`scan_paths`] does the same for given files and folders (targets) only:
//! each is looked at with [`crate::FileSystem::stat`], a folder is walked, and
//! only the known tracks at or below a target may be updated or removed. A
//! target that no longer exists loses its tracks, unless its whole root is gone
//! (an unmounted drive): that is reported and changes nothing, as in the full
//! scan. Paths below a hidden name are ignored, as the walk ignores them.
//!
//! A file under a nested root belongs to the deepest root containing it. A known
//! file that stops parsing keeps its previous row (and is retried on the next scan);
//! its error is reported. Paths that are not valid UTF-8 are reported, not stored.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Component, MAIN_SEPARATOR, Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::artwork::{self, StoredArt};
use crate::fs::{FileEntry, PathKind, is_hidden};
use crate::normalize::normalize;
use crate::tags::{self, FileMeta};
use crate::util::absolute_components;
use crate::{Library, LibraryError, ScanProgress, ScanReport};

/// Changed files per write transaction.
pub(crate) const BATCH_SIZE: usize = 500;

struct Known {
    id: i64,
    root_id: i64,
    size: i64,
    mtime_ns: i64,
}

struct Upsert {
    root_id: i64,
    path: String,
    entry: FileEntry,
    meta: FileMeta,
    artwork: Option<StoredArt>,
    existed: bool,
}

enum Change {
    Upsert(Box<Upsert>),
    /// An unchanged file now belongs to a different (nested) root.
    Reroot {
        id: i64,
        root_id: i64,
    },
}

pub(crate) fn scan(
    lib: &Library,
    progress: &mut dyn FnMut(ScanProgress),
) -> Result<ScanReport, LibraryError> {
    let roots = lib.root_rows()?;
    let known = load_known(&lib.conn, None)?;
    let mut pass = Pass::new(lib);
    let mut seen: HashSet<String> = HashSet::new();
    let mut failed_roots: HashSet<i64> = HashSet::new();

    for (root_id, root) in &roots {
        let nested: Vec<&Path> = roots
            .iter()
            .filter(|(id, other)| id != root_id && other.starts_with(root))
            .map(|(_, other)| other.as_path())
            .collect();
        for item in lib.fs.walk(root) {
            let entry = match item {
                Ok(entry) => entry,
                Err(err) => {
                    failed_roots.insert(*root_id);
                    pass.report.errors.push((root.clone(), err.to_string()));
                    continue;
                }
            };
            if !tags::is_audio_path(&entry.path) || nested.iter().any(|inner| entry.path.starts_with(inner)) {
                continue;
            }
            let Some(path) = pass.utf8_path(&entry.path) else {
                continue;
            };
            if !seen.insert(path.clone()) {
                continue;
            }
            let prior = known.get(&path);
            pass.visit(*root_id, entry, path, prior, progress)?;
        }
    }
    pass.flush()?;

    let vanished: Vec<i64> = known
        .iter()
        .filter(|(path, k)| !seen.contains(*path) && !failed_roots.contains(&k.root_id))
        .map(|(_, k)| k.id)
        .collect();
    pass.remove(vanished)?;
    pass.finish("library scan finished")
}

pub(crate) fn scan_paths(
    lib: &Library,
    paths: &[PathBuf],
    progress: &mut dyn FnMut(ScanProgress),
) -> Result<ScanReport, LibraryError> {
    let roots = lib.root_rows()?;
    let mut pass = Pass::new(lib);
    for target in targets(&roots, paths, &mut pass.report) {
        pass.target(&roots, &target, progress)?;
    }
    pass.flush()?;
    pass.finish("library paths scanned")
}

/// The files and folders to scan for `paths`, sorted, each covering what is
/// below it: a path inside a root stands for itself, a path above roots for
/// those roots. Paths outside every root are reported.
fn targets(roots: &[(i64, PathBuf)], paths: &[PathBuf], report: &mut ScanReport) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    for path in paths {
        let clean = match absolute_components(path) {
            Ok(clean) => clean,
            Err(err) => {
                report.errors.push((path.clone(), err.to_string()));
                continue;
            }
        };
        if roots.iter().any(|(_, root)| clean.starts_with(root)) {
            found.push(clean);
            continue;
        }
        let below: Vec<PathBuf> = roots
            .iter()
            .filter(|(_, root)| root.starts_with(&clean))
            .map(|(_, root)| root.clone())
            .collect();
        if below.is_empty() {
            report.errors.push((clean, "not in a library folder".to_owned()));
        }
        found.extend(below);
    }
    found.sort();
    found.dedup();
    // Paths sort by component, so everything below a target follows it directly.
    let mut targets: Vec<PathBuf> = Vec::new();
    for path in found {
        if !targets.last().is_some_and(|target| path.starts_with(target)) {
            targets.push(path);
        }
    }
    targets
}

/// The deepest root containing `path`.
fn deepest_root<'r>(roots: &'r [(i64, PathBuf)], path: &Path) -> Option<&'r (i64, PathBuf)> {
    roots
        .iter()
        .filter(|(_, root)| path.starts_with(root))
        .max_by_key(|(_, root)| root.components().count())
}

/// Whether `path` is below a hidden name inside `root`.
fn hidden_below(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|below| {
        below
            .components()
            .any(|c| matches!(c, Component::Normal(name) if is_hidden(name)))
    })
}

fn size_i64(size: u64) -> i64 {
    i64::try_from(size).unwrap_or(i64::MAX)
}

/// Known tracks: all of them, or those at or below `prefix`.
fn load_known(conn: &Connection, prefix: Option<&str>) -> Result<HashMap<String, Known>, LibraryError> {
    let read = |row: &rusqlite::Row<'_>| {
        Ok((
            row.get::<_, String>(0)?,
            Known {
                id: row.get(1)?,
                root_id: row.get(2)?,
                size: row.get(3)?,
                mtime_ns: row.get(4)?,
            },
        ))
    };
    const COLUMNS: &str = "SELECT path, id, root_id, file_size, mtime_ns FROM tracks";
    let rows = match prefix {
        None => conn
            .prepare(COLUMNS)?
            .query_map([], read)?
            .collect::<Result<_, _>>()?,
        Some(prefix) => {
            // Paths below `prefix` sort in [prefix + '/', prefix + ('/' + 1)), byte-wise.
            let separator = MAIN_SEPARATOR as u8;
            let first = format!("{prefix}{MAIN_SEPARATOR}");
            let after = format!("{prefix}{}", char::from(separator + 1));
            conn.prepare_cached(&format!(
                "{COLUMNS} WHERE path = ?1 OR (path >= ?2 AND path < ?3)"
            ))?
            .query_map(params![prefix, first, after], read)?
            .collect::<Result<_, _>>()?
        }
    };
    Ok(rows)
}

/// One scan or path scan: visits files, writes changes in batches and
/// removes vanished tracks.
struct Pass<'l> {
    lib: &'l Library,
    writer: Writer<'l>,
    report: ScanReport,
    batch: Vec<Change>,
    files_seen: u64,
    files_changed: u64,
}

impl<'l> Pass<'l> {
    fn new(lib: &'l Library) -> Self {
        Self {
            lib,
            writer: Writer::new(&lib.conn),
            report: ScanReport::default(),
            batch: Vec::new(),
            files_seen: 0,
            files_changed: 0,
        }
    }

    /// The path as stored text; a path that is not UTF-8 is reported.
    fn utf8_path(&mut self, path: &Path) -> Option<String> {
        let text = path.to_str().map(str::to_owned);
        if text.is_none() {
            self.report
                .errors
                .push((path.to_path_buf(), "path is not valid UTF-8".to_owned()));
        }
        text
    }

    /// One audio file found by a walk or a stat: skipped when unchanged since
    /// `prior`, else parsed and queued.
    fn visit(
        &mut self,
        root_id: i64,
        entry: FileEntry,
        path: String,
        prior: Option<&Known>,
        progress: &mut dyn FnMut(ScanProgress),
    ) -> Result<(), LibraryError> {
        self.files_seen += 1;
        let unchanged = prior.filter(|k| k.size == size_i64(entry.size) && k.mtime_ns == entry.mtime_ns);
        if unchanged.is_none() {
            self.files_changed += 1;
        }
        progress(ScanProgress {
            files_seen: self.files_seen,
            files_changed: self.files_changed,
            current: entry.path.clone(),
        });

        if let Some(known) = unchanged {
            self.report.unchanged += 1;
            if known.root_id != root_id {
                self.batch.push(Change::Reroot {
                    id: known.id,
                    root_id,
                });
            }
        } else {
            match tags::parse(self.lib.fs.as_ref(), &entry.path, true) {
                Ok(parsed) => {
                    // Cached before the transaction; a failure costs the art, not the track.
                    let artwork = parsed.front_cover().and_then(|picture| {
                        artwork::store(&self.lib.artwork_dir(), picture.data(), picture.mime_type())
                            .map_err(|err| {
                                self.report
                                    .errors
                                    .push((entry.path.clone(), format!("cannot cache artwork: {err}")));
                            })
                            .ok()
                    });
                    self.batch.push(Change::Upsert(Box::new(Upsert {
                        root_id,
                        path,
                        meta: parsed.meta(),
                        artwork,
                        existed: prior.is_some(),
                        entry,
                    })));
                }
                Err(err) => {
                    tracing::debug!(path = %entry.path.display(), %err, "unreadable audio file");
                    self.report.errors.push((entry.path, err.to_string()));
                }
            }
        }
        if self.batch.len() >= BATCH_SIZE {
            self.flush()?;
        }
        Ok(())
    }

    /// Scans one target of [`scan_paths`].
    fn target(
        &mut self,
        roots: &[(i64, PathBuf)],
        target: &Path,
        progress: &mut dyn FnMut(ScanProgress),
    ) -> Result<(), LibraryError> {
        let Some((_, root)) = deepest_root(roots, target) else {
            return Ok(());
        };
        if hidden_below(root, target) {
            return Ok(());
        }
        let Some(key) = self.utf8_path(target) else {
            return Ok(());
        };
        let known = load_known(&self.lib.conn, Some(&key))?;
        let mut seen: HashSet<String> = HashSet::new();
        let mut failed = false;
        match self.lib.fs.stat(target) {
            Ok(PathKind::File(entry)) => self.found(roots, entry, &known, &mut seen, progress)?,
            Ok(PathKind::Dir) => {
                for item in self.lib.fs.walk(target) {
                    match item {
                        Ok(entry) => self.found(roots, entry, &known, &mut seen, progress)?,
                        Err(err) => {
                            failed = true;
                            self.report.errors.push((target.to_path_buf(), err.to_string()));
                        }
                    }
                }
            }
            Ok(PathKind::Other) => {}
            Err(err) if matches!(err.kind(), io::ErrorKind::NotFound | io::ErrorKind::NotADirectory) => {
                // Gone, unless the whole root is (an unmounted drive must not empty the library).
                if !matches!(self.lib.fs.stat(root), Ok(PathKind::Dir)) {
                    failed = true;
                    self.report
                        .errors
                        .push((root.clone(), "library folder is not available".to_owned()));
                }
            }
            Err(err) => {
                failed = true;
                self.report.errors.push((target.to_path_buf(), err.to_string()));
            }
        }
        if !failed {
            let vanished = known
                .iter()
                .filter(|(path, _)| !seen.contains(*path))
                .map(|(_, k)| k.id)
                .collect();
            self.remove(vanished)?;
        }
        Ok(())
    }

    /// A file found below a target: visited when it is audio, in its deepest root.
    fn found(
        &mut self,
        roots: &[(i64, PathBuf)],
        entry: FileEntry,
        known: &HashMap<String, Known>,
        seen: &mut HashSet<String>,
        progress: &mut dyn FnMut(ScanProgress),
    ) -> Result<(), LibraryError> {
        if !tags::is_audio_path(&entry.path) {
            return Ok(());
        }
        let Some(&(root_id, ref root)) = deepest_root(roots, &entry.path) else {
            return Ok(());
        };
        if hidden_below(root, &entry.path) {
            return Ok(());
        }
        let Some(path) = self.utf8_path(&entry.path) else {
            return Ok(());
        };
        if !seen.insert(path.clone()) {
            return Ok(());
        }
        let prior = known.get(&path);
        self.visit(root_id, entry, path, prior, progress)
    }

    /// Writes the queued changes.
    fn flush(&mut self) -> Result<(), LibraryError> {
        self.writer.apply(&mut self.batch, &mut self.report)
    }

    /// Removes tracks by id, in batches.
    fn remove(&mut self, mut ids: Vec<i64>) -> Result<(), LibraryError> {
        ids.sort_unstable();
        for chunk in ids.chunks(BATCH_SIZE) {
            self.writer.remove(chunk, &mut self.report)?;
        }
        Ok(())
    }

    /// Deletes what lost its last track, and logs the outcome.
    fn finish(self, what: &str) -> Result<ScanReport, LibraryError> {
        let report = self.report;
        if report.added + report.updated + report.removed > 0 {
            let tx = self.lib.conn.unchecked_transaction()?;
            let stale_art = delete_orphans(&tx)?;
            tx.commit()?;
            self.lib.delete_artwork_files(&stale_art);
        }
        tracing::info!(
            added = report.added,
            updated = report.updated,
            removed = report.removed,
            unchanged = report.unchanged,
            errors = report.errors.len(),
            "{what}"
        );
        Ok(report)
    }
}

/// Writes queued changes; caches artist and album ids for the length of a scan.
struct Writer<'c> {
    conn: &'c Connection,
    artists: HashMap<String, i64>,
    albums: HashMap<(String, String), i64>,
}

impl<'c> Writer<'c> {
    fn new(conn: &'c Connection) -> Self {
        Self {
            conn,
            artists: HashMap::new(),
            albums: HashMap::new(),
        }
    }

    fn apply(&mut self, batch: &mut Vec<Change>, report: &mut ScanReport) -> Result<(), LibraryError> {
        if batch.is_empty() {
            return Ok(());
        }
        let conn = self.conn;
        let tx = conn.unchecked_transaction()?;
        for change in batch.drain(..) {
            match change {
                Change::Reroot { id, root_id } => {
                    tx.prepare_cached("UPDATE tracks SET root_id = ?1 WHERE id = ?2")?
                        .execute(params![root_id, id])?;
                }
                Change::Upsert(upsert) => {
                    self.upsert(&tx, &upsert)?;
                    if upsert.existed {
                        report.updated += 1;
                    } else {
                        report.added += 1;
                    }
                }
            }
        }
        bump_revision(&tx)?;
        tx.commit()?;
        Ok(())
    }

    fn remove(&mut self, ids: &[i64], report: &mut ScanReport) -> Result<(), LibraryError> {
        let conn = self.conn;
        let tx = conn.unchecked_transaction()?;
        for id in ids {
            report.removed += tx
                .prepare_cached("DELETE FROM tracks WHERE id = ?1")?
                .execute([id])? as u64;
        }
        bump_revision(&tx)?;
        tx.commit()?;
        Ok(())
    }

    fn upsert(&mut self, conn: &Connection, upsert: &Upsert) -> Result<i64, LibraryError> {
        let meta = &upsert.meta;
        let title = meta
            .title
            .clone()
            .unwrap_or_else(|| file_stem(&upsert.entry.path));
        // Group by album artist, else the primary track artist, so "A feat. B" stays on A's album.
        let group_artist = meta
            .album_artist
            .as_deref()
            .or(meta.artists.first().map(String::as_str))
            .or(meta.artist.as_deref());
        let album_id = match &meta.album {
            Some(album) => self.album_id(conn, album, group_artist)?,
            None => None,
        };
        let now = unix_now();
        if let Some(art) = &upsert.artwork {
            conn.prepare_cached(
                "INSERT OR IGNORE INTO artwork (hash, file_name, mime, byte_len) VALUES (?1, ?2, ?3, ?4)",
            )?
            .execute(params![
                art.hash,
                art.file_name,
                art.mime,
                i64::try_from(art.byte_len).unwrap_or(i64::MAX)
            ])?;
        }
        let rg = &meta.replaygain;
        let id: i64 = conn
            .prepare_cached(
                "INSERT INTO tracks (
                    root_id, path, file_size, mtime_ns, title, artist, album, album_artist, album_id,
                    track_no, disc_no, year, genre, duration_ms, sample_rate, channels, bit_depth, codec,
                    rg_track_gain, rg_track_peak, rg_album_gain, rg_album_peak, isrc, has_embedded_lyrics,
                    artwork_hash, title_norm, artist_norm, album_norm, added_at, updated_at
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                    ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?29
                )
                ON CONFLICT (path) DO UPDATE SET
                    root_id = excluded.root_id, file_size = excluded.file_size,
                    mtime_ns = excluded.mtime_ns, title = excluded.title, artist = excluded.artist,
                    album = excluded.album, album_artist = excluded.album_artist,
                    album_id = excluded.album_id, track_no = excluded.track_no,
                    disc_no = excluded.disc_no, year = excluded.year, genre = excluded.genre,
                    duration_ms = excluded.duration_ms, sample_rate = excluded.sample_rate,
                    channels = excluded.channels, bit_depth = excluded.bit_depth,
                    codec = excluded.codec, rg_track_gain = excluded.rg_track_gain,
                    rg_track_peak = excluded.rg_track_peak, rg_album_gain = excluded.rg_album_gain,
                    rg_album_peak = excluded.rg_album_peak, isrc = excluded.isrc,
                    has_embedded_lyrics = excluded.has_embedded_lyrics,
                    artwork_hash = excluded.artwork_hash, title_norm = excluded.title_norm,
                    artist_norm = excluded.artist_norm, album_norm = excluded.album_norm,
                    updated_at = excluded.updated_at
                RETURNING id",
            )?
            .query_row(
                params![
                    upsert.root_id,
                    upsert.path,
                    size_i64(upsert.entry.size),
                    upsert.entry.mtime_ns,
                    title,
                    meta.artist,
                    meta.album,
                    meta.album_artist,
                    album_id,
                    meta.track_no,
                    meta.disc_no,
                    meta.year,
                    meta.genre,
                    meta.duration_ms.map(|ms| i64::try_from(ms).unwrap_or(i64::MAX)),
                    meta.sample_rate,
                    meta.channels,
                    meta.bit_depth,
                    meta.codec,
                    rg.track_gain_db,
                    rg.track_peak,
                    rg.album_gain_db,
                    rg.album_peak,
                    meta.isrc,
                    meta.has_lyrics,
                    upsert.artwork.as_ref().map(|art| art.hash.as_str()),
                    normalize(&title),
                    normalize(meta.artist.as_deref().unwrap_or_default()),
                    normalize(meta.album.as_deref().unwrap_or_default()),
                    now,
                ],
                |row| row.get(0),
            )?;

        conn.prepare_cached("DELETE FROM track_artists WHERE track_id = ?1")?
            .execute([id])?;
        for (position, name) in meta.artists.iter().enumerate() {
            if let Some(artist_id) = self.artist_id(conn, name)? {
                conn.prepare_cached(
                    "INSERT OR IGNORE INTO track_artists (track_id, artist_id, position) VALUES (?1, ?2, ?3)",
                )?
                .execute(params![id, artist_id, position as i64])?;
            }
        }
        Ok(id)
    }

    fn artist_id(&mut self, conn: &Connection, name: &str) -> Result<Option<i64>, LibraryError> {
        let key = normalize(name);
        if key.is_empty() {
            return Ok(None);
        }
        if let Some(&id) = self.artists.get(&key) {
            return Ok(Some(id));
        }
        let found: Option<i64> = conn
            .prepare_cached("SELECT id FROM artists WHERE name_norm = ?1")?
            .query_row([&key], |row| row.get(0))
            .optional()?;
        let id = match found {
            Some(id) => id,
            None => {
                conn.prepare_cached("INSERT INTO artists (name, name_norm) VALUES (?1, ?2)")?
                    .execute(params![name, key])?;
                conn.last_insert_rowid()
            }
        };
        self.artists.insert(key, id);
        Ok(Some(id))
    }

    fn album_id(
        &mut self,
        conn: &Connection,
        title: &str,
        artist: Option<&str>,
    ) -> Result<Option<i64>, LibraryError> {
        let key = (normalize(title), normalize(artist.unwrap_or_default()));
        if key.0.is_empty() {
            return Ok(None);
        }
        if let Some(&id) = self.albums.get(&key) {
            return Ok(Some(id));
        }
        let found: Option<i64> = conn
            .prepare_cached("SELECT id FROM albums WHERE title_norm = ?1 AND artist_norm = ?2")?
            .query_row(params![key.0, key.1], |row| row.get(0))
            .optional()?;
        let id = match found {
            Some(id) => id,
            None => {
                conn.prepare_cached(
                    "INSERT INTO albums (title, album_artist, title_norm, artist_norm) VALUES (?1, ?2, ?3, ?4)",
                )?
                .execute(params![title, artist, key.0, key.1])?;
                conn.last_insert_rowid()
            }
        };
        self.albums.insert(key, id);
        Ok(Some(id))
    }
}

/// Deletes albums, artists and artwork rows no track refers to any more.
/// Returns the artwork file names to delete from the cache after the commit.
pub(crate) fn delete_orphans(conn: &Connection) -> Result<Vec<String>, LibraryError> {
    conn.execute(
        "DELETE FROM albums WHERE NOT EXISTS (SELECT 1 FROM tracks WHERE tracks.album_id = albums.id)",
        [],
    )?;
    conn.execute(
        "DELETE FROM artists WHERE NOT EXISTS
            (SELECT 1 FROM track_artists WHERE track_artists.artist_id = artists.id)",
        [],
    )?;
    const ORPHAN_ART: &str =
        "FROM artwork WHERE NOT EXISTS (SELECT 1 FROM tracks WHERE tracks.artwork_hash = artwork.hash)";
    let files = conn
        .prepare(&format!("SELECT file_name {ORPHAN_ART}"))?
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    conn.execute(&format!("DELETE {ORPHAN_ART}"), [])?;
    Ok(files)
}

pub(crate) fn bump_revision(conn: &Connection) -> Result<(), LibraryError> {
    conn.prepare_cached("UPDATE meta SET value = value + 1 WHERE key = 'revision'")?
        .execute([])?;
    Ok(())
}

pub(crate) fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| PathBuf::from(path).display().to_string())
}
