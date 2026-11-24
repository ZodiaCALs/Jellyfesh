//! A source's Discover page (docs/ui.md "Discover", ADR-0022): the shelves
//! the core gives (`Event::Discover`), the titles of the shelves the core
//! makes worded by their stable ids, and See All as the folder it browses.

use jf_core::{ItemKind, Shelf, SourceItem};

/// A shelf the core makes, whose title the app words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Known {
    ForYou,
    RecentlyAdded,
    MostPlayed,
    Rediscover,
}

/// The shelf's title, when the core made it (else its own title shows).
pub fn known(shelf: &Shelf) -> Option<Known> {
    match shelf.id.as_str() {
        "for_you" => Some(Known::ForYou),
        "recently_added" => Some(Known::RecentlyAdded),
        "most_played" => Some(Known::MostPlayed),
        "rediscover" => Some(Known::Rediscover),
        _ => None,
    }
}

/// See All: the shelf's `more` node as a folder of the source, to browse.
pub fn see_all(plugin: &str, shelf: &Shelf) -> Option<SourceItem> {
    let node = shelf.more.clone()?;
    Some(SourceItem {
        source: plugin.to_owned(),
        kind: ItemKind::Folder,
        id: node,
        title: shelf.title.clone(),
        subtitle: None,
        album: None,
        duration_ms: None,
        artwork: None,
        playable: false,
        unplayable_reason: None,
        track: None,
        also_in: Vec::new(),
        link: None,
        artist_id: None,
        album_id: None,
        downloaded: false,
    })
}

/// Shelves with something on them, in the core's order.
pub fn shown(shelves: &[Shelf]) -> Vec<&Shelf> {
    shelves.iter().filter(|s| !s.items.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use jf_core::ShelfStyle;

    use super::*;

    fn shelf(id: &str, items: usize, more: Option<&str>) -> Shelf {
        let item = SourceItem {
            source: "subsonic".into(),
            kind: ItemKind::Album,
            id: "a".into(),
            title: "Lanterns".into(),
            subtitle: None,
            album: None,
            duration_ms: None,
            artwork: None,
            playable: true,
            unplayable_reason: None,
            track: None,
            also_in: Vec::new(),
            link: None,
            artist_id: None,
            album_id: None,
            downloaded: false,
        };
        Shelf {
            id: id.into(),
            title: "For You".into(),
            style: ShelfStyle::Row,
            items: vec![item; items],
            more: more.map(str::to_owned),
        }
    }

    #[test]
    fn the_cores_shelves_are_worded_by_id() {
        assert_eq!(known(&shelf("for_you", 1, None)), Some(Known::ForYou));
        assert_eq!(known(&shelf("rediscover", 1, None)), Some(Known::Rediscover));
        assert_eq!(known(&shelf("node-17", 1, None)), None);
    }

    #[test]
    fn see_all_browses_the_shelfs_node() {
        assert!(see_all("subsonic", &shelf("x", 1, None)).is_none());
        let folder = see_all("subsonic", &shelf("x", 1, Some("node-17"))).unwrap();
        assert_eq!(folder.kind, ItemKind::Folder);
        assert_eq!(folder.id, "node-17");
        assert_eq!(folder.source, "subsonic");
        assert_eq!(folder.title, "For You");
    }

    #[test]
    fn empty_shelves_are_left_out() {
        let shelves = [shelf("a", 0, None), shelf("b", 2, None)];
        let ids: Vec<&str> = shown(&shelves).iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["b"]);
    }
}
