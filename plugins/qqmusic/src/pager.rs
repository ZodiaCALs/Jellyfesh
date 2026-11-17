//! Pages of the size the host asks for, over lists that QQ Music pages by
//! number with a fixed page size it does not state.
//!
//! A request reads one or more *streams* (one per search type, or a single
//! playlist or chart) in rounds: round `r` takes server page `r` of every
//! stream still active, in order. A stream stays active for the next round
//! while the server says `hasMore`. When the host's limit falls inside a
//! server page, the cursor remembers the stream and the offset, so nothing is
//! dropped and nothing repeats. The last few server pages are cached, so
//! splitting a page costs no second request; a list read again from its
//! first page is fetched afresh ([`invalidate`]).

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;

/// Where the next host page starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    /// Server page number of the current round.
    pub round: u32,
    /// Streams (indices) read in this round, in order.
    pub active: Vec<u8>,
    /// Streams found to have more pages in this round so far.
    pub next: Vec<u8>,
    /// Position in `active`.
    pub at: usize,
    /// Items of the current server page already returned.
    pub offset: usize,
}

impl Cursor {
    /// The first page over streams `0..count`.
    pub fn start(streams: &[u8]) -> Cursor {
        Cursor {
            round: 0,
            active: streams.to_vec(),
            next: Vec::new(),
            at: 0,
            offset: 0,
        }
    }

    /// `r<round>.a<active>.n<next>.i<at>.o<offset>`, streams as digits.
    pub fn encode(&self) -> String {
        let digits = |list: &[u8]| list.iter().map(|s| char::from(b'0' + s)).collect::<String>();
        format!(
            "r{}.a{}.n{}.i{}.o{}",
            self.round,
            digits(&self.active),
            digits(&self.next),
            self.at,
            self.offset
        )
    }

    /// Reads a cursor this module wrote; `streams` is how many there are.
    pub fn decode(text: &str, streams: u8) -> Result<Cursor> {
        let bad = || PluginError::invalid_input(format!("not a page cursor: {text:?}"));
        let mut parts = text.split('.');
        let mut field = |prefix: char| -> Result<&str> {
            parts.next().and_then(|p| p.strip_prefix(prefix)).ok_or_else(bad)
        };
        let number = |s: &str| s.parse::<u32>().map_err(|_| bad());
        let list = |s: &str| -> Result<Vec<u8>> {
            s.bytes()
                .map(|b| match b.checked_sub(b'0') {
                    Some(d) if d < streams => Ok(d),
                    _ => Err(bad()),
                })
                .collect()
        };
        let cursor = Cursor {
            round: number(field('r')?)?,
            active: list(field('a')?)?,
            next: list(field('n')?)?,
            at: number(field('i')?)? as usize,
            offset: number(field('o')?)? as usize,
        };
        if parts.next().is_some() || cursor.at > cursor.active.len() || cursor.round > 10_000 {
            return Err(bad());
        }
        Ok(cursor)
    }
}

/// One server page: its items and whether the server has more.
pub type ServerPage = (Vec<Item>, bool);

/// Most server pages read for one host page (they cost requests).
const MAX_FETCHES: usize = 8;
/// Server pages kept in the cache.
const CACHE: usize = 12;

thread_local! {
    static PAGES: RefCell<Vec<(String, ServerPage)>> = const { RefCell::new(Vec::new()) };
}

/// A server page from the cache, or fetched and cached under `key`.
pub fn cached(key: String, fetch: impl FnOnce() -> Result<ServerPage>) -> Result<ServerPage> {
    if let Some(page) = PAGES.with(|p| p.borrow().iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())) {
        return Ok(page);
    }
    let page = fetch()?;
    PAGES.with(|p| {
        let mut pages = p.borrow_mut();
        if pages.len() >= CACHE {
            pages.remove(0);
        }
        pages.push((key, page.clone()));
    });
    Ok(page)
}

/// Forgets the cached pages (after signing in with another key).
pub fn forget() {
    PAGES.with(|p| p.borrow_mut().clear());
}

/// Forgets the cached pages whose key starts with `prefix`: a list read again
/// from its first page is read afresh, while its later pages still come from
/// the pages read for the earlier ones.
pub fn invalidate(prefix: &str) {
    PAGES.with(|p| p.borrow_mut().retain(|(key, _)| !key.starts_with(prefix)));
}

/// How much one host page may read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    /// Up to the host's limit, across rounds (a playlist's or chart's songs).
    Limit,
    /// At most to the end of a round: one server page of each stream (a
    /// search of several types, so a page costs one request per type).
    Round,
}

/// Collects up to `limit` items from `cursor` on. `fetch(stream, page)` reads
/// one server page of a stream.
pub fn collect(
    mut cursor: Cursor,
    limit: u32,
    fill: Fill,
    mut fetch: impl FnMut(u8, u32) -> Result<ServerPage>,
) -> Result<ItemPage> {
    let limit = limit.max(1) as usize;
    let mut items = Vec::new();
    let mut fetches = 0;
    loop {
        if cursor.at >= cursor.active.len() {
            if cursor.next.is_empty() {
                return Ok(ItemPage::new(items));
            }
            cursor = Cursor {
                round: cursor.round + 1,
                active: std::mem::take(&mut cursor.next),
                next: Vec::new(),
                at: 0,
                offset: 0,
            };
            if fill == Fill::Round && !items.is_empty() {
                return Ok(ItemPage::with_next(items, cursor.encode()));
            }
        }
        if items.len() >= limit || fetches >= MAX_FETCHES {
            return Ok(ItemPage::with_next(items, cursor.encode()));
        }
        let stream = cursor.active[cursor.at];
        let (page, more) = fetch(stream, cursor.round)?;
        fetches += 1;
        // A stream joins the next round once, when its page is first read; an
        // empty page ends it whatever the server says.
        if cursor.offset == 0 && more && !page.is_empty() {
            cursor.next.push(stream);
        }
        let start = cursor.offset.min(page.len());
        let take = (limit - items.len()).min(page.len() - start);
        items.extend_from_slice(&page[start..start + take]);
        cursor.offset = start + take;
        if cursor.offset >= page.len() {
            cursor.at += 1;
            cursor.offset = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stream `s` has `pages[s]` pages of `size` items named `s-page-index`.
    fn server(pages: &'static [u32], size: usize) -> impl FnMut(u8, u32) -> Result<ServerPage> {
        move |stream, page| {
            let count = pages[usize::from(stream)];
            if page >= count {
                return Ok((Vec::new(), false));
            }
            let items = (0..size)
                .map(|i| Item::new(ItemKind::Track, format!("{stream}-{page}-{i}"), "t"))
                .collect();
            Ok((items, page + 1 < count))
        }
    }

    fn ids(page: &ItemPage) -> Vec<String> {
        page.items.iter().map(|i| i.id.clone()).collect()
    }

    #[test]
    fn cursors_round_trip_and_reject_junk() {
        let c = Cursor {
            round: 3,
            active: vec![0, 2],
            next: vec![2],
            at: 1,
            offset: 7,
        };
        assert_eq!(c.encode(), "r3.a02.n2.i1.o7");
        assert_eq!(Cursor::decode(&c.encode(), 4).unwrap(), c);
        for junk in [
            "",
            "r1",
            "r1.a0.n.i0.o0.x",
            "r1.a9.n.i0.o0",
            "r1.a0.n.i5.o0",
            "rX.a0.n.i0.o0",
        ] {
            assert!(Cursor::decode(junk, 4).is_err(), "{junk}");
        }
    }

    #[test]
    fn splits_server_pages_without_dropping_or_repeating() {
        let mut fetch = server(&[3], 10);
        let mut all = Vec::new();
        let mut cursor = Cursor::start(&[0]);
        loop {
            let page = collect(cursor.clone(), 4, Fill::Limit, &mut fetch).unwrap();
            all.extend(ids(&page));
            match page.next {
                Some(next) => cursor = Cursor::decode(&next, 1).unwrap(),
                None => break,
            }
        }
        assert_eq!(all.len(), 30);
        assert_eq!(all[0], "0-0-0");
        assert_eq!(all[29], "0-2-9");
        let mut unique = all.clone();
        unique.dedup();
        assert_eq!(unique.len(), 30);
    }

    #[test]
    fn several_streams_go_round_by_round() {
        // Songs have two pages, singers one.
        let page = collect(Cursor::start(&[0, 1]), 50, Fill::Limit, server(&[2, 1], 3)).unwrap();
        assert_eq!(
            ids(&page),
            [
                "0-0-0", "0-0-1", "0-0-2", "1-0-0", "1-0-1", "1-0-2", "0-1-0", "0-1-1", "0-1-2"
            ]
        );
        assert_eq!(page.next, None);
        // With a small limit the same items arrive over several pages.
        let first = collect(Cursor::start(&[0, 1]), 4, Fill::Limit, server(&[2, 1], 3)).unwrap();
        assert_eq!(ids(&first), ["0-0-0", "0-0-1", "0-0-2", "1-0-0"]);
        let next = Cursor::decode(first.next.as_deref().unwrap(), 2).unwrap();
        let second = collect(next, 4, Fill::Limit, server(&[2, 1], 3)).unwrap();
        assert_eq!(ids(&second), ["1-0-1", "1-0-2", "0-1-0", "0-1-1"]);
    }

    #[test]
    fn a_round_page_ends_with_its_round() {
        let first = collect(Cursor::start(&[0, 1]), 50, Fill::Round, server(&[2, 1], 3)).unwrap();
        assert_eq!(
            ids(&first),
            ["0-0-0", "0-0-1", "0-0-2", "1-0-0", "1-0-1", "1-0-2"]
        );
        let next = Cursor::decode(first.next.as_deref().unwrap(), 2).unwrap();
        let second = collect(next, 50, Fill::Round, server(&[2, 1], 3)).unwrap();
        assert_eq!(ids(&second), ["0-1-0", "0-1-1", "0-1-2"]);
        assert_eq!(second.next, None);
    }

    #[test]
    fn stops_after_a_few_requests() {
        // Endless one-item pages: the page ends after MAX_FETCHES requests.
        let page = collect(Cursor::start(&[0]), 200, Fill::Limit, server(&[1000], 1)).unwrap();
        assert_eq!(page.items.len(), MAX_FETCHES);
        assert!(page.next.is_some());
    }
}
