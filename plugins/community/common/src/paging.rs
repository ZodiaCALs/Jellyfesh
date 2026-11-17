//! Cursors that are plain offsets, and whole lists cut into pages.

use jf_plugin_sdk::{Item, ItemPage, PageRequest, PluginError, Result};

/// The most items a page may carry (the host's limit).
pub const MAX_PAGE: u32 = 200;

/// The offset a cursor stands for; no cursor is the start.
pub fn offset(page: &PageRequest) -> Result<usize> {
    match page.cursor.as_deref() {
        None | Some("") => Ok(0),
        Some(cursor) => cursor
            .parse()
            .map_err(|_| PluginError::invalid_input(format!("bad cursor {cursor:?}"))),
    }
}

/// The page size asked for, within 1 and [`MAX_PAGE`].
pub fn limit(page: &PageRequest) -> usize {
    page.limit.clamp(1, MAX_PAGE) as usize
}

/// A page of a whole list, with the cursor of the next page and the total.
pub fn slice(items: Vec<Item>, page: &PageRequest) -> Result<ItemPage> {
    let start = offset(page)?;
    let total = items.len();
    let end = start.saturating_add(limit(page)).min(total);
    let chunk: Vec<Item> = items
        .into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect();
    Ok(ItemPage {
        items: chunk,
        next: (end < total).then(|| end.to_string()),
        total: u32::try_from(total).ok(),
    })
}

/// A page the service cut itself: `items` start at `start`, `total` is the
/// whole list's length when the service says.
pub fn served(items: Vec<Item>, start: usize, asked: usize, total: Option<usize>) -> ItemPage {
    let end = start + items.len();
    let more = match total {
        Some(total) => end < total && !items.is_empty(),
        None => items.len() >= asked && asked > 0,
    };
    ItemPage {
        items,
        next: more.then(|| end.to_string()),
        total: total.and_then(|t| u32::try_from(t).ok()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_plugin_sdk::ItemKind;

    fn page(cursor: Option<&str>, limit: u32) -> PageRequest {
        PageRequest {
            cursor: cursor.map(str::to_string),
            limit,
        }
    }

    fn items(n: usize) -> Vec<Item> {
        (0..n)
            .map(|i| Item::new(ItemKind::Track, format!("tr:{i}"), "t"))
            .collect()
    }

    #[test]
    fn slices_whole_lists() {
        let first = slice(items(5), &page(None, 2)).unwrap();
        assert_eq!(first.items.len(), 2);
        assert_eq!((first.next.as_deref(), first.total), (Some("2"), Some(5)));
        let last = slice(items(5), &page(Some("4"), 2)).unwrap();
        assert_eq!((last.items.len(), last.next), (1, None));
        let past = slice(items(5), &page(Some("9"), 2)).unwrap();
        assert!(past.items.is_empty() && past.next.is_none());
        assert!(slice(items(5), &page(Some("x"), 2)).is_err());
        assert_eq!(slice(items(500), &page(None, 1000)).unwrap().items.len(), 200);
    }

    #[test]
    fn pages_cut_by_the_service() {
        let page = served(items(3), 6, 3, Some(9));
        assert_eq!((page.next, page.total), (None, Some(9)));
        let page = served(items(3), 0, 3, Some(9));
        assert_eq!(page.next.as_deref(), Some("3"));
        let page = served(items(3), 0, 3, None);
        assert_eq!(page.next.as_deref(), Some("3"));
        let page = served(items(2), 0, 3, None);
        assert_eq!(page.next, None);
        let page = served(Vec::new(), 3, 3, Some(9));
        assert_eq!(page.next, None, "an empty page ends the list");
    }
}
