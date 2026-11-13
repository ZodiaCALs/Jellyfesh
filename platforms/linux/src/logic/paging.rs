//! Page bookkeeping for lazily filled list models.
//!
//! Long lists (songs, albums, artists, playlist entries, the queue) never ask
//! the core for everything: a list model knows the item count, and when the
//! view binds an item whose page has not been fetched, it fetches that page
//! off the main thread. A reset (the library changed) starts a new
//! generation, so answers to old requests are dropped.

use std::collections::HashSet;
use std::ops::Range;

/// Items per page: a few screens of rows, one query each.
pub const PAGE_SIZE: u32 = 120;

/// The page holding item `index`.
pub fn page_of(index: u32, page_size: u32) -> u32 {
    index / page_size.max(1)
}

/// The item indices of `page` in a list of `total` items.
pub fn page_range(page: u32, page_size: u32, total: u32) -> Range<u32> {
    let start = page.saturating_mul(page_size).min(total);
    let end = start.saturating_add(page_size).min(total);
    start..end
}

/// Which pages are loaded or on their way.
#[derive(Debug, Default)]
pub struct Pages {
    requested: HashSet<u32>,
    generation: u64,
}

impl Pages {
    pub fn new() -> Self {
        Self::default()
    }

    /// The current generation; tag requests with it.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Marks `page` requested; true when it must be fetched now.
    pub fn request(&mut self, page: u32) -> bool {
        self.requested.insert(page)
    }

    /// The pages to fetch when item `index` is shown: its own and, near the
    /// end of a page, the next one (prefetch while scrolling down).
    pub fn wanted(&mut self, index: u32, page_size: u32, total: u32) -> Vec<u32> {
        let page = page_of(index, page_size);
        let mut out = Vec::with_capacity(2);
        if self.request(page) {
            out.push(page);
        }
        let in_page = index % page_size.max(1);
        let next = page + 1;
        if in_page >= page_size - page_size / 4
            && !page_range(next, page_size, total).is_empty()
            && self.request(next)
        {
            out.push(next);
        }
        out
    }

    /// True when an answer tagged with `generation` still applies.
    pub fn accepts(&self, generation: u64) -> bool {
        generation == self.generation
    }

    /// Forgets every page (the data changed); returns the new generation.
    pub fn reset(&mut self) -> u64 {
        self.requested.clear();
        self.generation += 1;
        self.generation
    }

    /// A failed fetch may be retried later.
    pub fn failed(&mut self, page: u32, generation: u64) {
        if self.accepts(generation) {
            self.requested.remove(&page);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_math() {
        assert_eq!(page_of(0, 100), 0);
        assert_eq!(page_of(99, 100), 0);
        assert_eq!(page_of(100, 100), 1);
        assert_eq!(page_range(0, 100, 250), 0..100);
        assert_eq!(page_range(2, 100, 250), 200..250);
        assert_eq!(page_range(3, 100, 250), 250..250);
        assert_eq!(page_of(5, 0), 5);
    }

    #[test]
    fn requests_each_page_once_and_prefetches() {
        let mut p = Pages::new();
        assert_eq!(p.wanted(3, 100, 1_000), vec![0]);
        assert_eq!(p.wanted(4, 100, 1_000), Vec::<u32>::new());
        // Near the end of page 0 the next page comes along.
        assert_eq!(p.wanted(80, 100, 1_000), vec![1]);
        assert_eq!(p.wanted(150, 100, 1_000), Vec::<u32>::new());
        // No prefetch past the end of the list.
        assert_eq!(p.wanted(990, 100, 1_000), vec![9]);
    }

    #[test]
    fn generations_drop_stale_answers() {
        let mut p = Pages::new();
        assert!(p.request(0));
        let old = p.generation();
        let new = p.reset();
        assert!(!p.accepts(old));
        assert!(p.accepts(new));
        assert!(p.request(0), "a reset forgets requested pages");
        p.failed(0, old);
        assert!(!p.request(0), "a stale failure does not clear the new request");
        p.failed(0, new);
        assert!(p.request(0), "a failed page can be fetched again");
    }
}
