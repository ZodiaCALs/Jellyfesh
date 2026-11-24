//! Search by source (ADR-0024 §4), with the core's rules:
//!
//! * every source that can search now is asked, merging or not, each kind
//!   on its own (`search-filters.kinds`: songs, albums, artists, playlists),
//!   [`PAGE`] of each on the first page;
//! * the library answers first. A source whose terms allow it
//!   (`content_policy.unified_search`) is merged with the library and the
//!   merging sources that answered before it ([`crate::sources::sections`],
//!   `jf_plugin_model::merge`): its group on the All tab holds what it adds,
//!   each entry where its first playable copy is, with the others in "also
//!   in";
//! * a source whose terms forbid merging keeps its results apart: never
//!   de-duplicated against other sources and never beside their rows. The
//!   All tab links to its tab ("Results from {Service}");
//! * every source that answered with something has a tab: its songs,
//!   albums, artists and playlists as it listed them, each with Show More
//!   while the source has more of that kind (its cursor).
//!
//! The names follow the core's search events (`SearchUpdate.merged`,
//! `SearchGroup { kind, items, next, total }`, `SearchMore`). The page runs
//! the calls (and the deadline) and hands each source's answer to a
//! [`Search`]; [`Search::view`] and [`Search::tab`] are what the Search page
//! shows.

use jf_plugin_model::{ContentPolicy, Item, ItemKind, ItemPage};
use serde::Serialize;

use crate::sources::{SectionRow, sections};

/// What is asked of each source, kind by kind, in this order.
pub const KINDS: [ItemKind; 4] = [
    ItemKind::Track,
    ItemKind::Album,
    ItemKind::Artist,
    ItemKind::Playlist,
];
/// How many of each kind a page asks for.
pub const PAGE: u32 = 20;
/// How many of each kind the library answers with.
pub const LIBRARY_LIMIT: usize = 50;

/// The id of the library's answer among the sources' (as in the core).
pub const LIBRARY: &str = "library";

/// What the page asks each source: these kinds, each on its own, `limit` of each.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Plan {
    pub kinds: Vec<ItemKind>,
    pub limit: u32,
    pub library_limit: usize,
}

pub fn plan() -> Plan {
    Plan {
        kinds: KINDS.to_vec(),
        limit: PAGE,
        library_limit: LIBRARY_LIMIT,
    }
}

/// Where a source's search stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Searching,
    Done,
    Failed,
}

/// One kind of one source's results (the core's `SearchGroup`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchGroup {
    pub kind: ItemKind,
    pub items: Vec<Item>,
    /// Where Show More goes on from; `None` when there is no more.
    pub next: Option<String>,
    /// How many there are in all, when the source says.
    pub total: Option<u32>,
}

#[derive(Debug, Clone)]
struct SourceResults {
    id: String,
    name: String,
    /// Its terms allow merging its results with others'.
    merged: bool,
    status: Status,
    error: Option<String>,
    groups: Vec<SearchGroup>,
    /// When it answered among the sources (the order of the sections and tabs).
    answered: Option<usize>,
}

impl SourceResults {
    fn items(&self) -> Vec<Item> {
        self.groups.iter().flat_map(|k| k.items.iter().cloned()).collect()
    }

    fn found(&self) -> bool {
        self.groups.iter().any(|k| !k.items.is_empty())
    }
}

/// A merging source's section on the All tab (From Your Sources).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Section {
    pub source: String,
    pub name: String,
    pub status: Status,
    pub error: Option<String>,
    pub rows: Vec<SectionRow>,
}

/// A source with a tab of its own.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TabLink {
    pub source: String,
    pub name: String,
    /// Its results are merged on the All tab; otherwise the All tab links to
    /// this tab ("Results from {Service}").
    pub merged: bool,
}

/// What the All and Library tabs show, and which tabs there are.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct View {
    /// The library's songs, albums and artists, with the merging sources
    /// that have them too.
    pub library: Vec<SectionRow>,
    /// From Your Sources: the merging sources, in the order they answered,
    /// then those still searching or that failed.
    pub sections: Vec<Section>,
    /// The sources that answered with something, in the order they
    /// answered: one tab each.
    pub tabs: Vec<TabLink>,
    /// Every source has answered, failed or run out of time.
    pub done: bool,
    /// Nothing anywhere, and nothing more to wait for.
    pub empty: bool,
}

/// A source's tab.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tab {
    pub source: String,
    pub name: String,
    pub merged: bool,
    /// Songs, albums, artists and playlists, those with results.
    pub groups: Vec<SearchGroup>,
}

/// One query's results. See the module documentation.
#[derive(Debug, Clone, Default)]
pub struct Search {
    library: Vec<Item>,
    sources: Vec<SourceResults>,
    answered: usize,
}

impl Search {
    /// A search with the library's answer (its songs, albums and artists).
    pub fn new(library: Vec<Item>) -> Search {
        Search {
            library,
            sources: Vec::new(),
            answered: 0,
        }
    }

    /// A source is being asked.
    pub fn asking(&mut self, id: &str, name: &str) {
        if self.source(id).is_some() {
            return;
        }
        self.sources.push(SourceResults {
            id: id.to_owned(),
            name: name.to_owned(),
            merged: false,
            status: Status::Searching,
            error: None,
            groups: Vec::new(),
            answered: None,
        });
    }

    /// A source answered, with a first page for each kind it was asked
    /// (`(kind, page)`; a kind it could not answer is left out). Its
    /// content policy says whether its results may be merged.
    pub fn answer(&mut self, id: &str, policy: &ContentPolicy, pages: Vec<(ItemKind, ItemPage)>) {
        let order = self.answered;
        let Some(source) = self.source_mut(id) else {
            return;
        };
        if source.status != Status::Searching {
            return;
        }
        source.merged = policy.unified_search;
        source.status = Status::Done;
        source.groups = KINDS
            .iter()
            .filter_map(|kind| {
                let (_, page) = pages.iter().find(|(k, _)| k == kind)?;
                Some(SearchGroup {
                    kind: *kind,
                    items: unique(page.items.iter().filter(|i| i.kind == *kind).cloned(), &[]),
                    next: page.next.clone(),
                    total: page.total,
                })
            })
            .collect();
        source.answered = Some(order);
        self.answered += 1;
    }

    /// A source failed, or ran out of time, with why.
    pub fn fail(&mut self, id: &str, error: &str) {
        if let Some(source) = self.source_mut(id)
            && source.status == Status::Searching
        {
            source.status = Status::Failed;
            source.error = Some(error.to_owned());
        }
    }

    /// The cursor for Show More of a source's `kind`, or `None`.
    pub fn cursor(&self, id: &str, kind: ItemKind) -> Option<String> {
        self.source(id)?
            .groups
            .iter()
            .find(|k| k.kind == kind)?
            .next
            .clone()
    }

    /// Show More: the next page of a source's `kind`. Items it already
    /// listed are left out.
    pub fn more(&mut self, id: &str, kind: ItemKind, page: ItemPage) {
        let Some(source) = self.source_mut(id) else {
            return;
        };
        let Some(results) = source.groups.iter_mut().find(|k| k.kind == kind) else {
            return;
        };
        let added = unique(page.items.into_iter().filter(|i| i.kind == kind), &results.items);
        results.items.extend(added);
        results.next = page.next;
    }

    /// The All and Library tabs, and the list of tabs.
    pub fn view(&self) -> View {
        let mut answered: Vec<&SourceResults> =
            self.sources.iter().filter(|s| s.answered.is_some()).collect();
        answered.sort_by_key(|s| s.answered);
        let merging: Vec<&SourceResults> = answered.iter().copied().filter(|s| s.merged).collect();
        let mut answers: Vec<(String, Vec<Item>)> = vec![(LIBRARY.to_owned(), self.library.clone())];
        answers.extend(merging.iter().map(|s| (s.id.clone(), s.items())));
        let mut merged = sections(&answers).into_iter();
        let library = merged.next().unwrap_or_default();
        let mut sections: Vec<Section> = merging
            .iter()
            .zip(merged)
            .map(|(source, rows)| Section {
                source: source.id.clone(),
                name: source.name.clone(),
                status: Status::Done,
                error: None,
                rows,
            })
            .collect();
        // Still searching, or failed: shown after the answers (whether a source merges is
        // known only from its answer).
        sections.extend(
            self.sources
                .iter()
                .filter(|s| s.answered.is_none())
                .map(|s| Section {
                    source: s.id.clone(),
                    name: s.name.clone(),
                    status: s.status.clone(),
                    error: s.error.clone(),
                    rows: Vec::new(),
                }),
        );
        let tabs: Vec<TabLink> = answered
            .iter()
            .filter(|s| s.found())
            .map(|s| TabLink {
                source: s.id.clone(),
                name: s.name.clone(),
                merged: s.merged,
            })
            .collect();
        let done = self.sources.iter().all(|s| s.status != Status::Searching);
        let empty = done && library.is_empty() && tabs.is_empty();
        View {
            library,
            sections,
            tabs,
            done,
            empty,
        }
    }

    /// A source's tab, or `None` when it has none.
    pub fn tab(&self, id: &str) -> Option<Tab> {
        let source = self.source(id).filter(|s| s.found())?;
        Some(Tab {
            source: source.id.clone(),
            name: source.name.clone(),
            merged: source.merged,
            groups: source
                .groups
                .iter()
                .filter(|k| !k.items.is_empty())
                .cloned()
                .collect(),
        })
    }

    fn source(&self, id: &str) -> Option<&SourceResults> {
        self.sources.iter().find(|s| s.id == id)
    }

    fn source_mut(&mut self, id: &str) -> Option<&mut SourceResults> {
        self.sources.iter_mut().find(|s| s.id == id)
    }
}

/// `items` without repeats (by id), and without those already in `listed`.
fn unique(items: impl Iterator<Item = Item>, listed: &[Item]) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    for item in items {
        if !listed.iter().chain(out.iter()).any(|i| i.id == item.id) {
            out.push(item);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: ItemKind, id: &str, title: &str, artist: &str) -> Item {
        Item {
            id: id.into(),
            kind,
            title: title.into(),
            artist: Some(artist.into()),
            album: None,
            album_id: None,
            artist_id: None,
            duration_ms: (kind == ItemKind::Track).then_some(200_000),
            track_number: None,
            disc_number: None,
            year: None,
            child_count: None,
            isrc: None,
            artwork: None,
            explicit: false,
            playable: true,
            unplayable: None,
        }
    }

    fn page(items: Vec<Item>, next: Option<&str>) -> ItemPage {
        ItemPage {
            items,
            next: next.map(str::to_owned),
            total: None,
        }
    }

    fn policy(unified_search: bool) -> ContentPolicy {
        ContentPolicy {
            unified_search,
            ..ContentPolicy::default()
        }
    }

    fn ids(rows: &[SectionRow]) -> Vec<&str> {
        rows.iter().map(|r| r.item.id.as_str()).collect()
    }

    #[test]
    fn asks_every_kind_on_its_own() {
        let plan = plan();
        assert_eq!(plan.kinds, KINDS);
        assert_eq!(plan.limit, 20);
    }

    #[test]
    fn the_library_comes_first_and_merging_sources_add_to_it() {
        let lanterns = item(ItemKind::Track, "lib-3", "Lanterns", "Glass Harbour");
        let mut search = Search::new(vec![lanterns]);
        search.asking("a", "Subsonic");
        search.asking("b", "Jellyfin");
        let view = search.view();
        assert!(!view.done);
        assert_eq!(view.sections.len(), 2);
        assert!(view.sections.iter().all(|g| g.status == Status::Searching));
        // Jellyfin answers first: its own copy of the library's song joins it.
        search.answer(
            "b",
            &policy(true),
            vec![
                (
                    ItemKind::Track,
                    page(
                        vec![item(ItemKind::Track, "j3", "Lanterns", "Glass Harbour")],
                        None,
                    ),
                ),
                (
                    ItemKind::Album,
                    page(
                        vec![item(ItemKind::Album, "j1", "Slow Tide", "Glass Harbour")],
                        Some("2"),
                    ),
                ),
            ],
        );
        search.answer(
            "a",
            &policy(true),
            vec![(
                ItemKind::Track,
                page(
                    vec![
                        item(ItemKind::Track, "s3", "Lanterns", "Glass Harbour"),
                        item(ItemKind::Track, "s5", "Night Ferry", "Tidal Choir"),
                    ],
                    None,
                ),
            )],
        );
        let view = search.view();
        assert!(view.done);
        assert!(!view.empty);
        assert_eq!(ids(&view.library), ["lib-3"]);
        assert_eq!(view.library[0].also_in, ["b", "a"]);
        let sections: Vec<(&str, Vec<&str>)> = view
            .sections
            .iter()
            .map(|g| (g.source.as_str(), ids(&g.rows)))
            .collect();
        assert_eq!(
            sections,
            [("b", vec!["j1"]), ("a", vec!["s5"])],
            "in the order they answered"
        );
        let tabs: Vec<&str> = view.tabs.iter().map(|t| t.source.as_str()).collect();
        assert_eq!(tabs, ["b", "a"]);
    }

    #[test]
    fn separate_sources_are_never_merged() {
        let mut search = Search::new(vec![item(ItemKind::Track, "lib-3", "Lanterns", "Glass Harbour")]);
        search.asking("spotify", "Spotify");
        search.asking("a", "Subsonic");
        search.answer(
            "spotify",
            &policy(false),
            vec![(
                ItemKind::Track,
                page(
                    vec![
                        item(ItemKind::Track, "sp3", "Lanterns", "Glass Harbour"),
                        item(ItemKind::Track, "sp5", "Night Ferry", "Tidal Choir"),
                    ],
                    Some("c1"),
                ),
            )],
        );
        search.answer(
            "a",
            &policy(true),
            vec![(
                ItemKind::Track,
                page(
                    vec![item(ItemKind::Track, "s5", "Night Ferry", "Tidal Choir")],
                    None,
                ),
            )],
        );
        let view = search.view();
        assert!(
            view.library[0].also_in.is_empty(),
            "the library's song is not 'also in' Spotify"
        );
        let groups: Vec<&str> = view.sections.iter().map(|g| g.source.as_str()).collect();
        assert_eq!(groups, ["a"], "no group beside the others on the All tab");
        assert_eq!(
            ids(&view.sections[0].rows),
            ["s5"],
            "not de-duplicated against Spotify"
        );
        let tabs: Vec<(&str, bool)> = view.tabs.iter().map(|t| (t.source.as_str(), t.merged)).collect();
        assert_eq!(tabs, [("spotify", false), ("a", true)]);
        let tab = search.tab("spotify").unwrap();
        assert_eq!(tab.groups.len(), 1);
        assert_eq!(
            tab.groups[0].items.len(),
            2,
            "its own tab lists everything it found"
        );
        assert_eq!(search.cursor("spotify", ItemKind::Track).as_deref(), Some("c1"));
    }

    #[test]
    fn show_more_pages_one_kind() {
        let mut search = Search::new(Vec::new());
        search.asking("a", "Subsonic");
        search.answer(
            "a",
            &policy(true),
            vec![
                (
                    ItemKind::Track,
                    page(vec![item(ItemKind::Track, "1", "One", "X")], Some("t2")),
                ),
                (ItemKind::Artist, page(Vec::new(), None)),
            ],
        );
        assert_eq!(search.cursor("a", ItemKind::Track).as_deref(), Some("t2"));
        assert_eq!(search.cursor("a", ItemKind::Artist), None);
        search.more(
            "a",
            ItemKind::Track,
            page(
                vec![
                    item(ItemKind::Track, "1", "One", "X"),
                    item(ItemKind::Track, "2", "Two", "X"),
                    item(ItemKind::Album, "9", "Stray", "X"),
                ],
                None,
            ),
        );
        let tab = search.tab("a").unwrap();
        assert_eq!(tab.groups.len(), 1, "kinds without results are left out");
        let songs: Vec<&str> = tab.groups[0].items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(songs, ["1", "2"], "repeats and other kinds are left out");
        assert_eq!(search.cursor("a", ItemKind::Track), None);
        assert!(search.tab("nobody").is_none());
    }

    #[test]
    fn failures_and_nothing_found() {
        let mut search = Search::new(Vec::new());
        search.asking("a", "Subsonic");
        search.asking("b", "Jellyfin");
        search.fail("a", "Subsonic did not answer in time.");
        search.answer(
            "b",
            &policy(true),
            vec![(ItemKind::Track, page(Vec::new(), None))],
        );
        search.answer(
            "a",
            &policy(true),
            vec![(
                ItemKind::Track,
                page(vec![item(ItemKind::Track, "1", "x", "y")], None),
            )],
        );
        let view = search.view();
        assert!(view.done);
        assert!(view.empty, "a late answer after a failure is dropped");
        let failed = view.sections.iter().find(|g| g.source == "a").unwrap();
        assert_eq!(failed.status, Status::Failed);
        assert_eq!(failed.error.as_deref(), Some("Subsonic did not answer in time."));
        assert!(view.tabs.is_empty(), "no tab for a source that found nothing");
    }
}
