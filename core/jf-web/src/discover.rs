//! Discover pages (ADR-0022 §2), as the core makes them (`jf-core/src/discover.rs`, the
//! reference). A source with a `discover` export gives its own shelves. For one without, a page is
//! put together from what the source has: "For You" (`for_you`) from
//! `recommendations.for-user`, then a shelf for each of the first six folders and playlists its
//! browse root lists (the node's id; See All browses it), and the other items of the root in one
//! grid (`browse`, titled with the source's name). The browser's plugin host makes the calls; this
//! decides what to ask for and puts the answers together. Shelves made here have stable ids, which
//! the page words by id.

use std::collections::BTreeMap;

use jf_plugin_model::{Item, ItemKind, Shelf, ShelfStyle};
use serde::{Deserialize, Serialize};

/// How long one plugin call for a Discover page may take, ms.
pub const CALL_MS: u32 = 25_000;
/// Items on a shelf made here.
pub const SHELF_ITEMS: u32 = 20;
/// Items read from the browse root.
pub const ROOT_ITEMS: u32 = 50;
/// Root folders made into shelves.
pub const ROOT_SHELVES: usize = 6;

/// A Discover page: its shelves, or why there are none.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DiscoverPage {
    pub shelves: Vec<Shelf>,
    pub error: Option<String>,
    /// What See All browses, by shelf id, for each shelf with `more`: the folder the shelf was
    /// made from, an item of that id listed on the page, or a bare folder (as the core's node
    /// lookup).
    pub see_all: BTreeMap<String, Item>,
    /// Made here rather than the plugin's own: its shelves are worded by id (`for_you`).
    pub made: bool,
}

/// The kinds a node id is looked up as, in order (the core's `NODE_KINDS`).
const NODE_KINDS: [ItemKind; 4] = [
    ItemKind::Album,
    ItemKind::Playlist,
    ItemKind::Artist,
    ItemKind::Folder,
];

/// A folder known only by its id, titled with its shelf's title.
fn bare_folder(id: &str, title: &str) -> Item {
    Item {
        id: id.to_owned(),
        kind: ItemKind::Folder,
        title: title.to_owned(),
        artist: None,
        album: None,
        album_id: None,
        artist_id: None,
        duration_ms: None,
        track_number: None,
        disc_number: None,
        year: None,
        child_count: None,
        isrc: None,
        artwork: None,
        explicit: false,
        playable: false,
        unplayable: None,
    }
}

/// Fills in `see_all` from `known` (the items the page's answers listed).
fn with_see_all(mut page: DiscoverPage, known: &[Item]) -> DiscoverPage {
    for shelf in &page.shelves {
        let Some(more) = &shelf.more else { continue };
        let node = NODE_KINDS
            .iter()
            .find_map(|kind| known.iter().find(|i| &i.id == more && i.kind == *kind))
            .cloned()
            .unwrap_or_else(|| bare_folder(more, &shelf.title));
        page.see_all.insert(shelf.id.clone(), node);
    }
    page
}

/// One call's answer: the items, or why there are none.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    Ok(Vec<Item>),
    Err(String),
}

/// The answers a made page is put together from.
#[derive(Debug, Clone, Deserialize)]
pub struct Answers {
    /// `recommendations.for-user`, when the source has it.
    #[serde(default)]
    pub for_you: Option<Answer>,
    /// The browse root, when the source can browse.
    #[serde(default)]
    pub root: Option<Answer>,
    /// Each folder of [`root_folders`] with its first items.
    #[serde(default)]
    pub folders: Vec<(Item, Answer)>,
}

/// The plugin's own shelves, without empty ones.
pub fn own_page(shelves: Vec<Shelf>) -> DiscoverPage {
    let shelves: Vec<Shelf> = shelves.into_iter().filter(|s| !s.items.is_empty()).collect();
    let known: Vec<Item> = shelves.iter().flat_map(|s| s.items.iter().cloned()).collect();
    with_see_all(
        DiscoverPage {
            shelves,
            ..DiscoverPage::default()
        },
        &known,
    )
}

fn is_folder(item: &Item) -> bool {
    matches!(item.kind, ItemKind::Folder | ItemKind::Playlist)
}

/// The folders and playlists of a browse root that become shelves: the first six.
pub fn root_folders(root: &[Item]) -> Vec<Item> {
    root.iter()
        .filter(|i| is_folder(i))
        .take(ROOT_SHELVES)
        .cloned()
        .collect()
}

/// The page made from a source's answers; `name` titles the grid of the root's other items.
pub fn made_page(name: &str, answers: Answers) -> DiscoverPage {
    let mut page = DiscoverPage {
        made: true,
        ..DiscoverPage::default()
    };
    let mut errors = Vec::new();
    let mut known = Vec::new();
    match answers.for_you {
        Some(Answer::Ok(items)) if !items.is_empty() => page.shelves.push(Shelf {
            id: "for_you".into(),
            title: "For You".into(),
            style: ShelfStyle::Row,
            items: cut(items),
            more: None,
        }),
        Some(Answer::Err(e)) => errors.push(e),
        _ => {}
    }
    match answers.root {
        Some(Answer::Ok(root)) => {
            let wanted = root_folders(&root);
            for (folder, answer) in answers.folders {
                // Only the folders asked for, in the root's order.
                if !wanted.iter().any(|w| w.id == folder.id) {
                    continue;
                }
                match answer {
                    Answer::Ok(items) if !items.is_empty() => {
                        page.shelves.push(Shelf {
                            id: folder.id.clone(),
                            title: folder.title.clone(),
                            style: ShelfStyle::Row,
                            items: cut(items),
                            more: Some(folder.id.clone()),
                        });
                        known.push(folder);
                    }
                    Answer::Ok(_) => {}
                    Answer::Err(e) => errors.push(e),
                }
            }
            let loose: Vec<Item> = root.into_iter().filter(|i| !is_folder(i)).collect();
            if !loose.is_empty() {
                page.shelves.push(Shelf {
                    id: "browse".into(),
                    title: name.to_owned(),
                    style: ShelfStyle::Grid,
                    items: loose,
                    more: None,
                });
            }
        }
        Some(Answer::Err(e)) => errors.push(e),
        None => {}
    }
    if page.shelves.is_empty() {
        page.error = Some(
            errors
                .into_iter()
                .next()
                .unwrap_or_else(|| format!("{name} has nothing to discover")),
        );
    }
    with_see_all(page, &known)
}

fn cut(mut items: Vec<Item>) -> Vec<Item> {
    items.truncate(SHELF_ITEMS as usize);
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, kind: ItemKind) -> Item {
        serde_json::from_value(serde_json::json!({
            "id": id, "kind": kind, "title": format!("Title {id}"), "explicit": false, "playable": true,
        }))
        .unwrap()
    }

    fn ok(items: Vec<Item>) -> Answer {
        Answer::Ok(items)
    }

    #[test]
    fn a_plugins_own_shelves_lose_the_empty_ones() {
        let shelf = |id: &str, items| Shelf {
            id: id.into(),
            title: id.into(),
            style: ShelfStyle::Chart,
            items,
            more: None,
        };
        let mut top = shelf(
            "top",
            vec![item("1", ItemKind::Track), item("al", ItemKind::Album)],
        );
        top.more = Some("charts".into());
        let mut albums = shelf("new", vec![item("al", ItemKind::Album)]);
        albums.more = Some("al".into());
        let page = own_page(vec![top, shelf("none", vec![]), albums]);
        assert_eq!(page.shelves.len(), 2);
        assert_eq!(page.shelves[0].id, "top");
        assert_eq!(page.error, None);
        assert!(!page.made);
        // See All: a bare folder titled with the shelf, or the listed item of that id.
        assert_eq!(page.see_all["top"].id, "charts");
        assert_eq!(page.see_all["top"].kind, ItemKind::Folder);
        assert_eq!(page.see_all["top"].title, "top");
        assert_eq!(page.see_all["new"].kind, ItemKind::Album);
    }

    #[test]
    fn a_page_is_made_from_suggestions_and_the_browse_root() {
        // As jf-core's a_page_is_made_for_sources_without_their_own: For You, then a shelf per
        // root folder (See All browses it), then the loose items as one grid.
        let root = vec![
            item("albums", ItemKind::Folder),
            item("a1", ItemKind::Album),
            item("mix", ItemKind::Playlist),
            item("empty", ItemKind::Folder),
        ];
        let folders = root_folders(&root);
        assert_eq!(
            folders.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(),
            ["albums", "mix", "empty"]
        );
        let many: Vec<Item> = (0..30).map(|i| item(&format!("t{i}"), ItemKind::Track)).collect();
        let page = made_page(
            "Mock",
            Answers {
                for_you: Some(ok(vec![item("daily", ItemKind::Playlist)])),
                root: Some(ok(root)),
                folders: vec![
                    (folders[0].clone(), ok(many)),
                    (folders[1].clone(), ok(vec![item("m1", ItemKind::Track)])),
                    (folders[2].clone(), ok(vec![])),
                ],
            },
        );
        let ids: Vec<&str> = page.shelves.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["for_you", "albums", "mix", "browse"]);
        assert_eq!(page.shelves[0].title, "For You");
        assert!(page.made);
        assert_eq!(page.shelves[0].style, ShelfStyle::Row);
        assert_eq!(page.shelves[1].items.len(), SHELF_ITEMS as usize);
        assert_eq!(
            page.shelves[1].more.as_deref(),
            Some("albums"),
            "See All browses the folder"
        );
        assert_eq!(page.see_all["albums"].kind, ItemKind::Folder);
        assert_eq!(
            page.see_all["mix"].kind,
            ItemKind::Playlist,
            "the root's own item, kind and all"
        );
        assert!(!page.see_all.contains_key("for_you"));
        assert_eq!(page.shelves[3].title, "Mock");
        assert_eq!(page.shelves[3].style, ShelfStyle::Grid);
        assert_eq!(page.shelves[3].items.len(), 1);
        assert_eq!(page.error, None);
    }

    #[test]
    fn only_the_first_six_folders_become_shelves() {
        let root: Vec<Item> = (0..9).map(|i| item(&format!("f{i}"), ItemKind::Folder)).collect();
        assert_eq!(root_folders(&root).len(), ROOT_SHELVES);
        let extra = (item("f8", ItemKind::Folder), ok(vec![item("x", ItemKind::Track)]));
        let page = made_page(
            "Mock",
            Answers {
                for_you: None,
                root: Some(ok(root)),
                folders: vec![extra],
            },
        );
        assert!(
            page.shelves.is_empty(),
            "a folder that was not asked for is left out"
        );
    }

    #[test]
    fn without_shelves_the_first_error_or_nothing_to_discover() {
        let page = made_page(
            "Mock",
            Answers {
                for_you: Some(Answer::Err("not signed in".into())),
                root: Some(Answer::Err("timed out".into())),
                folders: vec![],
            },
        );
        assert!(page.shelves.is_empty());
        assert_eq!(page.error.as_deref(), Some("not signed in"));
        let page = made_page(
            "Mock",
            Answers {
                for_you: Some(ok(vec![])),
                root: None,
                folders: vec![],
            },
        );
        assert_eq!(page.error.as_deref(), Some("Mock has nothing to discover"));
    }

    #[test]
    fn answers_read_from_the_pages_json() {
        let answers: Answers = serde_json::from_value(serde_json::json!({
            "for_you": {"err": "offline"},
            "root": {"ok": [{"id": "f", "kind": "Folder", "title": "F", "explicit": false, "playable": false}]},
            "folders": [[{"id": "f", "kind": "Folder", "title": "F", "explicit": false, "playable": false}, {"ok": []}]],
        }))
        .unwrap();
        let page = made_page("Mock", answers);
        assert_eq!(page.error.as_deref(), Some("offline"));
    }
}
