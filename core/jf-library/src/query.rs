//! Read queries: rows to public types.

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::{Album, AlbumSort, Artist, LibraryError, ReplayGain, Track, TrackId, TrackQuery, TrackSort};

/// The [`Track`] columns of `tracks t`, in the order [`track_at`] reads them.
pub(crate) const TRACK_COLUMNS: &str = "t.id, t.path, t.title, t.artist, t.album, t.album_artist, t.album_id,
    t.track_no, t.disc_no, t.year, t.genre, t.duration_ms, t.sample_rate, t.channels, t.bit_depth,
    t.codec, t.rg_track_gain, t.rg_track_peak, t.rg_album_gain, t.rg_album_peak, t.isrc,
    t.has_embedded_lyrics, t.artwork_hash, t.file_size, t.mtime_ns";

fn track_from_row(row: &Row<'_>) -> rusqlite::Result<Track> {
    track_at(row, 0)
}

/// A track from [`TRACK_COLUMNS`] starting at column `base`.
pub(crate) fn track_at(row: &Row<'_>, base: usize) -> rusqlite::Result<Track> {
    let col = |offset: usize| base + offset;
    Ok(Track {
        id: row.get(col(0))?,
        path: row.get::<_, String>(col(1))?.into(),
        title: row.get(col(2))?,
        artist: row.get(col(3))?,
        album: row.get(col(4))?,
        album_artist: row.get(col(5))?,
        album_id: row.get(col(6))?,
        track_no: row.get(col(7))?,
        disc_no: row.get(col(8))?,
        year: row.get(col(9))?,
        genre: row.get(col(10))?,
        duration_ms: row.get::<_, Option<i64>>(col(11))?.map(non_negative),
        sample_rate: row.get(col(12))?,
        channels: row.get(col(13))?,
        bit_depth: row.get(col(14))?,
        codec: row.get(col(15))?,
        replaygain: ReplayGain {
            track_gain_db: row.get::<_, Option<f64>>(col(16))?.map(|v| v as f32),
            track_peak: row.get::<_, Option<f64>>(col(17))?.map(|v| v as f32),
            album_gain_db: row.get::<_, Option<f64>>(col(18))?.map(|v| v as f32),
            album_peak: row.get::<_, Option<f64>>(col(19))?.map(|v| v as f32),
        },
        isrc: row.get(col(20))?,
        has_embedded_lyrics: row.get(col(21))?,
        artwork_hash: row.get(col(22))?,
        file_size: non_negative(row.get(col(23))?),
        mtime_ns: row.get(col(24))?,
    })
}

/// Like [`track_at`] for a `LEFT JOIN tracks t`: `None` when no track matched.
pub(crate) fn joined_track_at(row: &Row<'_>, base: usize) -> rusqlite::Result<Option<Track>> {
    match row.get::<_, Option<i64>>(base)? {
        Some(_) => track_at(row, base).map(Some),
        None => Ok(None),
    }
}

pub(crate) fn track(conn: &Connection, id: TrackId) -> Result<Option<Track>, LibraryError> {
    Ok(conn
        .prepare_cached(&format!("SELECT {TRACK_COLUMNS} FROM tracks t WHERE t.id = ?1"))?
        .query_row([id], track_from_row)
        .optional()?)
}

pub(crate) fn tracks(conn: &Connection, q: &TrackQuery) -> Result<Vec<Track>, LibraryError> {
    let dir = if q.descending { "DESC" } else { "ASC" };
    // Unknown values sort last in both directions; the remaining keys follow `dir`.
    let (nulls_last, keys): (&str, &[&str]) = match q.sort {
        TrackSort::Title => ("", &["t.title_norm", "t.id"]),
        TrackSort::Artist => (
            "t.artist IS NULL, ",
            &[
                "t.artist_norm",
                "t.album_norm",
                "t.disc_no",
                "t.track_no",
                "t.title_norm",
                "t.id",
            ],
        ),
        TrackSort::Album => (
            "t.album IS NULL, ",
            &[
                "t.album_norm",
                "t.album_id",
                "t.disc_no",
                "t.track_no",
                "t.title_norm",
                "t.id",
            ],
        ),
        TrackSort::Year => (
            "t.year IS NULL, ",
            &[
                "t.year",
                "t.album_norm",
                "t.disc_no",
                "t.track_no",
                "t.title_norm",
                "t.id",
            ],
        ),
        TrackSort::Duration => ("t.duration_ms IS NULL, ", &["t.duration_ms", "t.id"]),
        TrackSort::Path => ("", &["t.path"]),
        TrackSort::Added => ("", &["t.added_at", "t.id"]),
    };
    let order: Vec<String> = keys.iter().map(|key| format!("{key} {dir}")).collect();
    let sql = format!(
        "SELECT {TRACK_COLUMNS} FROM tracks t
         WHERE (?1 IS NULL OR t.album_id = ?1)
           AND (?2 IS NULL OR t.id IN (SELECT track_id FROM track_artists WHERE artist_id = ?2))
         ORDER BY {nulls_last}{}
         LIMIT ?3 OFFSET ?4",
        order.join(", ")
    );
    let rows = conn
        .prepare_cached(&sql)?
        .query_map(
            params![q.album_id, q.artist_id, to_i64(q.limit), to_i64(q.offset)],
            track_from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The latest-added tracks first.
pub(crate) fn recently_added(conn: &Connection, limit: usize) -> Result<Vec<Track>, LibraryError> {
    let sql = format!("SELECT {TRACK_COLUMNS} FROM tracks t ORDER BY t.added_at DESC, t.id DESC LIMIT ?1");
    let rows = conn
        .prepare_cached(&sql)?
        .query_map([to_i64(limit)], track_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// An album's tracks by disc (none counts as disc 1), track number (none last) and title.
pub(crate) fn album_tracks(conn: &Connection, album_id: i64) -> Result<Vec<Track>, LibraryError> {
    let sql = format!(
        "SELECT {TRACK_COLUMNS} FROM tracks t WHERE t.album_id = ?1
         ORDER BY IFNULL(t.disc_no, 1), t.track_no IS NULL, t.track_no, t.title_norm, t.id"
    );
    let rows = conn
        .prepare_cached(&sql)?
        .query_map([album_id], track_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The [`Album`] columns of `albums a`, as [`album_from_row`] reads them;
/// `year` is the earliest year among the album's tracks, and ORDER BY clauses
/// use the alias.
macro_rules! album_columns {
    () => {
        "a.id, a.title, a.album_artist,
        (SELECT MIN(year) FROM tracks WHERE album_id = a.id) AS year,
        (SELECT COUNT(*) FROM tracks WHERE album_id = a.id),
        (SELECT artwork_hash FROM tracks WHERE album_id = a.id AND artwork_hash IS NOT NULL
            ORDER BY disc_no, track_no, id LIMIT 1)"
    };
}

const ALBUM_SELECT: &str = concat!("SELECT ", album_columns!(), " FROM albums a");

/// Albums with a track credited to the artist, or grouped under the artist's
/// name (the album artist, or the primary track artist without one); by year
/// (unknown last), then title.
pub(crate) fn artist_albums(conn: &Connection, artist_id: i64) -> Result<Vec<Album>, LibraryError> {
    let sql = format!(
        "{ALBUM_SELECT}
         WHERE a.id IN (SELECT t.album_id FROM track_artists ta JOIN tracks t ON t.id = ta.track_id
                        WHERE ta.artist_id = ?1)
            OR a.artist_norm = (SELECT name_norm FROM artists WHERE id = ?1)
         ORDER BY year IS NULL, year, a.title_norm, a.id"
    );
    let rows = conn
        .prepare_cached(&sql)?
        .query_map([artist_id], album_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// A page of albums. Unknown artists and years sort last in both directions.
///
/// The inner query computes only the sort key `k` of every album; the album
/// columns, three lookups each, are computed for the page alone.
pub(crate) fn albums_sorted(
    conn: &Connection,
    sort: AlbumSort,
    descending: bool,
    offset: usize,
    limit: usize,
) -> Result<Vec<Album>, LibraryError> {
    const YEAR: &str = "(SELECT MIN(year) FROM tracks WHERE album_id = a.id)";
    const ADDED: &str = "(SELECT MIN(added_at) FROM tracks WHERE album_id = a.id)";
    let d = if descending { "DESC" } else { "ASC" };
    let (key, order) = match sort {
        AlbumSort::Title => ("NULL", format!("k_title {d}, k_artist {d}, k_id {d}")),
        AlbumSort::Artist => (
            YEAR,
            format!("k_artist = '', k_artist {d}, k IS NULL, k {d}, k_title {d}, k_id {d}"),
        ),
        AlbumSort::Year => (
            YEAR,
            format!("k IS NULL, k {d}, k_title {d}, k_artist {d}, k_id {d}"),
        ),
        AlbumSort::Added => (ADDED, format!("k {d}, k_id {d}")),
    };
    let sql = format!(
        "SELECT {} FROM (
            SELECT a.id AS k_id, a.title_norm AS k_title, a.artist_norm AS k_artist, {key} AS k
            FROM albums a ORDER BY {order} LIMIT ?1 OFFSET ?2
         ) JOIN albums a ON a.id = k_id
         ORDER BY {order}",
        album_columns!()
    );
    let rows = conn
        .prepare_cached(&sql)?
        .query_map(params![to_i64(limit), to_i64(offset)], album_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn album_from_row(row: &Row<'_>) -> rusqlite::Result<Album> {
    Ok(Album {
        id: row.get(0)?,
        title: row.get(1)?,
        album_artist: row.get(2)?,
        year: row.get(3)?,
        track_count: row.get(4)?,
        artwork_hash: row.get(5)?,
    })
}

pub(crate) fn album(conn: &Connection, id: i64) -> Result<Option<Album>, LibraryError> {
    Ok(conn
        .prepare_cached(&format!("{ALBUM_SELECT} WHERE a.id = ?1"))?
        .query_row([id], album_from_row)
        .optional()?)
}

const ARTIST_SELECT: &str = "SELECT ar.id, ar.name,
        (SELECT COUNT(*) FROM track_artists WHERE artist_id = ar.id)
    FROM artists ar";

fn artist_from_row(row: &Row<'_>) -> rusqlite::Result<Artist> {
    Ok(Artist {
        id: row.get(0)?,
        name: row.get(1)?,
        track_count: row.get(2)?,
    })
}

pub(crate) fn artists(conn: &Connection, offset: usize, limit: usize) -> Result<Vec<Artist>, LibraryError> {
    let sql = format!("{ARTIST_SELECT} ORDER BY ar.name_norm, ar.id LIMIT ?1 OFFSET ?2");
    let rows = conn
        .prepare_cached(&sql)?
        .query_map(params![to_i64(limit), to_i64(offset)], artist_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(crate) fn artist(conn: &Connection, id: i64) -> Result<Option<Artist>, LibraryError> {
    Ok(conn
        .prepare_cached(&format!("{ARTIST_SELECT} WHERE ar.id = ?1"))?
        .query_row([id], artist_from_row)
        .optional()?)
}

/// SQLite integers are signed; sizes and durations are stored non-negative.
pub(crate) fn non_negative(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

pub(crate) fn to_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}
