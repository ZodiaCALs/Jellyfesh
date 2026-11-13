//! Favourites and play statistics, one `item_stats` row per item key.
//!
//! Keys: the path of a library track or file (so a track keeps its counts when
//! it is scanned in again, and a file and the track scanned from it share
//! them), and (plugin, id) for plugin items. A row that is neither a favourite
//! nor played or skipped any more is deleted.

use rusqlite::{Connection, OptionalExtension, params};

use crate::item::{self, ITEM_COLUMN_COUNT, ITEM_COLUMNS, StoredItem};
use crate::query::{self, TRACK_COLUMNS, non_negative, to_i64};
use crate::scan::bump_revision;
use crate::util::unix_now_ms;
use crate::{ItemStats, LibraryError, PlaylistItem};

/// The row of a stored item: (id, favourite time).
fn find(conn: &Connection, item: &StoredItem) -> Result<Option<(i64, Option<i64>)>, LibraryError> {
    let read = |row: &rusqlite::Row<'_>| Ok((row.get(0)?, row.get(1)?));
    Ok(match (&item.path, &item.plugin, &item.source_id) {
        (Some(path), _, _) => conn
            .prepare_cached("SELECT id, favorite_at_ms FROM item_stats WHERE path = ?1")?
            .query_row([path], read)
            .optional()?,
        (None, Some(plugin), Some(id)) => conn
            .prepare_cached("SELECT id, favorite_at_ms FROM item_stats WHERE plugin = ?1 AND source_id = ?2")?
            .query_row([plugin, id], read)
            .optional()?,
        _ => None,
    })
}

/// The row for `item`, created empty if needed; records how the item was last referred to.
fn upsert(conn: &Connection, item: &StoredItem) -> Result<(i64, Option<i64>), LibraryError> {
    if let Some((id, favorite)) = find(conn, item)? {
        conn.prepare_cached("UPDATE item_stats SET kind = ?2, track_id = ?3, meta_json = ?4 WHERE id = ?1")?
            .execute(params![id, item.kind, item.track_id, item.meta_json])?;
        return Ok((id, favorite));
    }
    conn.prepare_cached(
        "INSERT INTO item_stats (kind, path, track_id, plugin, source_id, meta_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(params![
        item.kind,
        item.path,
        item.track_id,
        item.plugin,
        item.source_id,
        item.meta_json
    ])?;
    Ok((conn.last_insert_rowid(), None))
}

pub(crate) fn set_favorite(
    conn: &Connection,
    item: &PlaylistItem,
    favorite: bool,
) -> Result<(), LibraryError> {
    let tx = conn.unchecked_transaction()?;
    let stored = item::resolve(&tx, item)?;
    let changed = match (find(&tx, &stored)?, favorite) {
        (Some((_, Some(_))), true) | (Some((_, None)) | None, false) => false,
        (_, true) => {
            let (id, _) = upsert(&tx, &stored)?;
            // Later than every other favourite, so the list keeps the order of the
            // calls even within one millisecond or when the clock steps back.
            tx.execute(
                "UPDATE item_stats SET favorite_at_ms =
                    (SELECT MAX(?2, IFNULL(MAX(favorite_at_ms) + 1, 0)) FROM item_stats)
                 WHERE id = ?1",
                params![id, unix_now_ms()],
            )?;
            true
        }
        (Some((id, Some(_))), false) => {
            tx.execute("UPDATE item_stats SET favorite_at_ms = NULL WHERE id = ?1", [id])?;
            tx.execute(
                "DELETE FROM item_stats WHERE id = ?1 AND play_count = 0 AND skip_count = 0",
                [id],
            )?;
            true
        }
    };
    if changed {
        bump_revision(&tx)?;
    }
    tx.commit()?;
    Ok(())
}

pub(crate) fn is_favorite(conn: &Connection, item: &PlaylistItem) -> Result<bool, LibraryError> {
    let stored = match item::resolve(conn, item) {
        Ok(stored) => stored,
        Err(LibraryError::TrackNotFound(_)) => return Ok(false),
        Err(err) => return Err(err),
    };
    Ok(find(conn, &stored)?.is_some_and(|(_, favorite)| favorite.is_some()))
}

pub(crate) fn record_play(
    conn: &Connection,
    item: &PlaylistItem,
    at_ms: i64,
    completed: bool,
) -> Result<(), LibraryError> {
    let tx = conn.unchecked_transaction()?;
    let stored = item::resolve(&tx, item)?;
    let (id, _) = upsert(&tx, &stored)?;
    if completed {
        tx.execute(
            "UPDATE item_stats SET play_count = play_count + 1,
                last_played_ms = MAX(IFNULL(last_played_ms, ?2), ?2)
             WHERE id = ?1",
            params![id, at_ms],
        )?;
    } else {
        tx.execute(
            "UPDATE item_stats SET skip_count = skip_count + 1 WHERE id = ?1",
            [id],
        )?;
    }
    bump_revision(&tx)?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn item_key(conn: &Connection, item: &PlaylistItem) -> Result<Option<String>, LibraryError> {
    match item::resolve(conn, item) {
        Ok(stored) => Ok(Some(stored.key())),
        Err(LibraryError::TrackNotFound(_)) => Ok(None),
        Err(err) => Err(err),
    }
}

/// Rows of `item_stats x` matching `filter`, in `order`.
fn select(
    conn: &Connection,
    filter: &str,
    order: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<ItemStats>, LibraryError> {
    let sql = format!(
        "SELECT x.favorite_at_ms, x.play_count, x.skip_count, x.last_played_ms, {ITEM_COLUMNS}, {TRACK_COLUMNS}
         FROM item_stats x LEFT JOIN tracks t ON t.path = x.path
         WHERE {filter} ORDER BY {order} LIMIT ?1 OFFSET ?2"
    );
    let rows = conn
        .prepare_cached(&sql)?
        .query_map(params![to_i64(limit), to_i64(offset)], |row| {
            let track = query::joined_track_at(row, 4 + ITEM_COLUMN_COUNT)?;
            let favorited_at_ms: Option<i64> = row.get(0)?;
            Ok(ItemStats {
                item: item::item_at(row, 4, track.as_ref())?,
                track,
                favorite: favorited_at_ms.is_some(),
                favorited_at_ms,
                play_count: non_negative(row.get(1)?),
                skip_count: non_negative(row.get(2)?),
                last_played_ms: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(crate) fn favorites(
    conn: &Connection,
    offset: usize,
    limit: usize,
) -> Result<Vec<ItemStats>, LibraryError> {
    select(
        conn,
        "x.favorite_at_ms IS NOT NULL",
        "x.favorite_at_ms DESC, x.id DESC",
        offset,
        limit,
    )
}

pub(crate) fn most_played(conn: &Connection, limit: usize) -> Result<Vec<ItemStats>, LibraryError> {
    select(
        conn,
        "x.play_count > 0",
        "x.play_count DESC, x.last_played_ms DESC, x.id",
        0,
        limit,
    )
}

pub(crate) fn recently_played(conn: &Connection, limit: usize) -> Result<Vec<ItemStats>, LibraryError> {
    select(
        conn,
        "x.last_played_ms IS NOT NULL",
        "x.last_played_ms DESC, x.id DESC",
        0,
        limit,
    )
}
