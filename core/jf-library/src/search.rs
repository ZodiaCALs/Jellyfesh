//! Search over tracks, albums and artists.
//!
//! The query is normalised like the indexed columns (NFKC, lowercase, Latin
//! diacritics removed; see [`normalize`]) and split on whitespace into pieces.
//! Results are ranked in tiers, each tier only adding rows not already found:
//!
//! 1. exact title (artist name) match;
//! 2. title prefix match, as a range scan on the indexed normalised title;
//! 3. when some piece has 3 or more characters:
//!    * word and prefix matches from the `unicode61` index, every piece a prefix
//!      (`"piece"*`), ranked by bm25 with the title weighted highest;
//!    * then substring matches from the `trigram` index for the 3+ character
//!      pieces, with shorter pieces checked by `LIKE` on those hits;
//! 4. otherwise (only 1–2 character pieces, such as two-character CJK song titles,
//!    which trigrams cannot index; risk R13): a `LIKE '%piece%'` scan over the
//!    normalised columns, stopped by `LIMIT`.
//!
//! User text reaches FTS5 only inside double-quoted strings with quotes doubled,
//! so `AND`, `NEAR`, `*`, `:`, `^` and parentheses are matched as text, and `%`,
//! `_` and `\` are escaped in `LIKE` patterns.

use std::collections::HashSet;

use rusqlite::{Connection, ToSql, params_from_iter};

use crate::LibraryError;
use crate::normalize::normalize;
use crate::query::to_i64;

/// One searchable table and its two FTS5 indexes.
pub(crate) struct Target {
    table: &'static str,
    fts: &'static str,
    tri: &'static str,
    /// The normalised column ranked for exact and prefix matches.
    name: &'static str,
    /// Every normalised column searched.
    columns: &'static [&'static str],
    /// bm25 weights for `columns`.
    weights: &'static str,
}

pub(crate) const TRACKS: Target = Target {
    table: "tracks",
    fts: "tracks_fts",
    tri: "tracks_tri",
    name: "title_norm",
    columns: &["title_norm", "artist_norm", "album_norm"],
    weights: "10.0, 4.0, 2.0",
};

pub(crate) const ALBUMS: Target = Target {
    table: "albums",
    fts: "albums_fts",
    tri: "albums_tri",
    name: "title_norm",
    columns: &["title_norm", "artist_norm"],
    weights: "10.0, 3.0",
};

pub(crate) const ARTISTS: Target = Target {
    table: "artists",
    fts: "artists_fts",
    tri: "artists_tri",
    name: "name_norm",
    columns: &["name_norm"],
    weights: "1.0",
};

/// Pieces shorter than this cannot be matched by the trigram index.
const TRIGRAM: usize = 3;

/// A normalised query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Query {
    norm: String,
    pieces: Vec<String>,
}

impl Query {
    /// `None` for text that normalises to nothing.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let norm = normalize(text);
        let pieces = norm
            .split(' ')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect();
        (!norm.is_empty()).then_some(Self { norm, pieces })
    }

    fn is_short(&self) -> bool {
        self.pieces.iter().all(|piece| piece.chars().count() < TRIGRAM)
    }

    /// `"p1"* "p2"*`: every piece a prefix (implicit AND). Pieces with no letters or
    /// digits are left out: the tokenizer would make an empty phrase of them.
    fn word_match(&self) -> Option<String> {
        let parts: Vec<String> = self
            .pieces
            .iter()
            .filter(|piece| piece.chars().any(char::is_alphanumeric))
            .map(|piece| format!("{}*", fts_string(piece)))
            .collect();
        (!parts.is_empty()).then(|| parts.join(" "))
    }

    /// `"p1" "p2"`: every 3+ character piece as a substring.
    fn trigram_match(&self) -> Option<String> {
        let parts: Vec<String> = self
            .pieces
            .iter()
            .filter(|piece| piece.chars().count() >= TRIGRAM)
            .map(|piece| fts_string(piece))
            .collect();
        (!parts.is_empty()).then(|| parts.join(" "))
    }

    fn short_pieces(&self) -> impl Iterator<Item = &str> {
        self.pieces
            .iter()
            .map(String::as_str)
            .filter(|piece| piece.chars().count() < TRIGRAM)
    }
}

/// An FTS5 string literal: everything inside is text, never syntax.
fn fts_string(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// `%text%` with `%`, `_` and `\` escaped for `LIKE … ESCAPE '\'`.
fn like_pattern(text: &str) -> String {
    let mut pattern = String::with_capacity(text.len() + 2);
    pattern.push('%');
    for c in text.chars() {
        if matches!(c, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    pattern.push('%');
    pattern
}

/// `(x.a LIKE ?n ESCAPE '\' OR x.b LIKE ?n ESCAPE '\')`
fn any_column_like(columns: &[&str], param: usize) -> String {
    let parts: Vec<String> = columns
        .iter()
        .map(|column| format!("x.{column} LIKE ?{param} ESCAPE '\\'"))
        .collect();
    format!("({})", parts.join(" OR "))
}

/// Ids in rank order, deduplicated, at most `limit`.
struct Ranked {
    ids: Vec<i64>,
    seen: HashSet<i64>,
    limit: usize,
}

impl Ranked {
    fn full(&self) -> bool {
        self.ids.len() >= self.limit
    }

    fn extend(&mut self, ids: Vec<i64>) {
        for id in ids {
            if self.full() {
                break;
            }
            if self.seen.insert(id) {
                self.ids.push(id);
            }
        }
    }
}

/// Ids of `target` rows matching `query`, best first.
pub(crate) fn search(
    conn: &Connection,
    target: &Target,
    query: &Query,
    limit: usize,
) -> Result<Vec<i64>, LibraryError> {
    let Target {
        table,
        fts,
        tri,
        name,
        columns,
        weights,
    } = *target;
    let mut ranked = Ranked {
        ids: Vec::new(),
        seen: HashSet::new(),
        limit,
    };
    // Each tier fetches `limit` rows: enough even if all earlier hits recur.
    let n = to_i64(limit);

    let exact = format!("SELECT id FROM {table} WHERE {name} = ?1 ORDER BY id LIMIT ?2");
    ranked.extend(ids(conn, &exact, &[&query.norm, &n])?);

    if !ranked.full() {
        // Every string starting with the query sorts in [query, query + U+10FFFF).
        let upper = format!("{}\u{10FFFF}", query.norm);
        let prefix =
            format!("SELECT id FROM {table} WHERE {name} >= ?1 AND {name} < ?2 ORDER BY {name}, id LIMIT ?3");
        ranked.extend(ids(conn, &prefix, &[&query.norm, &upper, &n])?);
    }

    if !ranked.full() && !query.is_short() {
        if let Some(words) = query.word_match() {
            let sql = format!(
                "SELECT rowid FROM {fts} WHERE {fts} MATCH ?1 ORDER BY bm25({fts}, {weights}) LIMIT ?2"
            );
            ranked.extend(fts_ids(conn, &sql, &[&words, &n])?);
        }
        if !ranked.full()
            && let Some(substrings) = query.trigram_match()
        {
            let short: Vec<String> = query.short_pieces().map(like_pattern).collect();
            let mut sql =
                format!("SELECT x.id FROM {tri} JOIN {table} x ON x.id = {tri}.rowid WHERE {tri} MATCH ?1");
            for index in 0..short.len() {
                sql.push_str(" AND ");
                sql.push_str(&any_column_like(columns, index + 3));
            }
            sql.push_str(" LIMIT ?2");
            let mut params: Vec<&dyn ToSql> = vec![&substrings, &n];
            params.extend(short.iter().map(|p| p as &dyn ToSql));
            ranked.extend(fts_ids(conn, &sql, &params)?);
        }
    } else if !ranked.full() {
        let patterns: Vec<String> = query.pieces.iter().map(|p| like_pattern(p)).collect();
        let conditions: Vec<String> = (0..patterns.len())
            .map(|index| any_column_like(columns, index + 2))
            .collect();
        let sql = format!(
            "SELECT x.id FROM {table} x WHERE {} LIMIT ?1",
            conditions.join(" AND ")
        );
        let mut params: Vec<&dyn ToSql> = vec![&n];
        params.extend(patterns.iter().map(|p| p as &dyn ToSql));
        ranked.extend(ids(conn, &sql, &params)?);
    }
    Ok(ranked.ids)
}

fn ids(conn: &Connection, sql: &str, params: &[&dyn ToSql]) -> Result<Vec<i64>, LibraryError> {
    let rows = conn
        .prepare_cached(sql)?
        .query_map(params_from_iter(params.iter().copied()), |row| row.get(0))?
        .collect::<Result<Vec<i64>, _>>()?;
    Ok(rows)
}

/// Like [`ids`], but an FTS5 query error yields no hits instead of failing the
/// search. Quoting rules out syntax errors; this guards against tokenizer corner
/// cases with unusual scripts.
fn fts_ids(conn: &Connection, sql: &str, params: &[&dyn ToSql]) -> Result<Vec<i64>, LibraryError> {
    match ids(conn, sql, params) {
        Err(LibraryError::Database(rusqlite::Error::SqliteFailure(_, Some(message))))
            if message.contains("fts5") =>
        {
            tracing::warn!(%message, "FTS5 rejected a search query");
            Ok(Vec::new())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_user_text_for_fts5() {
        let q = Query::parse(r#"Say "Hi" NEAR(a*)"#).unwrap();
        assert_eq!(q.pieces, [r#"say"#, r#""hi""#, "near(a*)"]);
        assert_eq!(q.word_match().unwrap(), r#""say"* """hi"""* "near(a*)"*"#);
        assert_eq!(q.trigram_match().unwrap(), r#""say" """hi""" "near(a*)""#);
        assert_eq!(Query::parse(" \t "), None);
        assert_eq!(Query::parse("* -").unwrap().word_match(), None);
    }

    #[test]
    fn short_queries_and_like_escaping() {
        assert!(Query::parse("晴天").unwrap().is_short());
        assert!(Query::parse("杰伦 晴天").unwrap().is_short());
        assert!(!Query::parse("周杰伦 晴天").unwrap().is_short());
        assert_eq!(like_pattern(r"5%_a\b"), r"%5\%\_a\\b%");
    }
}
