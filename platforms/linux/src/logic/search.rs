//! Search by source (ADR-0024 §4), as it streams in: the tabs All, Library
//! and one per source that found something. All shows the library's
//! results, then From Your Sources (the sources whose terms allow merging),
//! then a link to the tab of each source whose terms forbid it ("Results
//! from Spotify"). A source's tab shows its results kind by kind, each with
//! Show More while it has a cursor.
//!
//! The core merges and de-duplicates (`SourceItem::also_in`, `merged`) and
//! pages (`SearchMore`); this only keeps the newest request's answers, the
//! tab shown and the Show More requests in flight.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use jf_core::{CommandId, ItemKind, SearchGroup, SearchPage, SearchUpdate, SourceItem};

/// The source id the core uses for the local library.
pub const LIBRARY: &str = "library";

/// A tab of the Search page.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum Tab {
    #[default]
    All,
    Library,
    /// A source that found something, by id.
    Source(String),
}

/// One source's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub source: String,
    /// Its rows on the All tab.
    pub items: Vec<SourceItem>,
    pub error: Option<String>,
    /// Part of the merged results (the library, From Your Sources); false
    /// keeps it apart, linked from All.
    pub merged: bool,
    /// Its own results kind by kind, for its tab.
    pub groups: Vec<SearchGroup>,
}

impl Section {
    /// Kinds present, in display order (songs first).
    pub fn kinds(&self) -> Vec<ItemKind> {
        [
            ItemKind::Track,
            ItemKind::Album,
            ItemKind::Artist,
            ItemKind::Playlist,
            ItemKind::Folder,
        ]
        .into_iter()
        .filter(|k| self.items.iter().any(|i| i.kind == *k))
        .collect()
    }

    /// What its tab shows: its groups that have results or, from a source
    /// that sent none, its rows kind by kind (without Show More).
    pub fn tab_groups(&self) -> Vec<SearchGroup> {
        if !self.groups.is_empty() {
            return self
                .groups
                .iter()
                .filter(|g| !g.items.is_empty())
                .cloned()
                .collect();
        }
        self.kinds()
            .into_iter()
            .map(|kind| SearchGroup {
                kind,
                items: self.items.iter().filter(|i| i.kind == kind).cloned().collect(),
                next: None,
                total: None,
            })
            .collect()
    }
}

/// The search page's state.
#[derive(Debug, Default)]
pub struct SearchState {
    query: String,
    request: Option<CommandId>,
    sections: Vec<Section>,
    done: bool,
    tab: Tab,
    /// Show More in flight: the source and kind each command asked for.
    more: HashMap<CommandId, (String, ItemKind)>,
    /// Why the last Show More of a source's kind failed.
    more_errors: HashMap<(String, ItemKind), String>,
}

impl SearchState {
    pub fn new() -> Self {
        Self::default()
    }

    /// A new search was sent: earlier answers no longer apply, and the page
    /// starts on All.
    pub fn start(&mut self, query: &str, request: CommandId) {
        *self = Self {
            query: query.to_owned(),
            request: Some(request),
            ..Self::default()
        };
    }

    /// Clears the page (empty query).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Takes in one source's answer; false when it belongs to an older
    /// request and was ignored.
    pub fn apply(&mut self, update: &Arc<SearchUpdate>) -> bool {
        if self.request != Some(update.request) {
            return false;
        }
        let section = Section {
            source: update.source.clone(),
            items: update.items.clone(),
            error: update.error.clone(),
            merged: update.merged,
            groups: update.groups.clone(),
        };
        match self.sections.iter_mut().find(|s| s.source == update.source) {
            Some(existing) => *existing = section,
            None if update.source == LIBRARY => self.sections.insert(0, section),
            None => self.sections.push(section),
        }
        self.done |= update.done;
        true
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// The search command whose answers are shown.
    pub fn request(&self) -> Option<CommandId> {
        self.request
    }

    /// Every answer, library first (the tests check the order).
    #[cfg(test)]
    fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// A source's answer, by id (`LIBRARY` for the library's).
    pub fn section(&self, source: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.source == source)
    }

    /// The tabs: All, Library, then each source that found something, in
    /// the order they answered; none before a search.
    pub fn tabs(&self) -> Vec<Tab> {
        if self.request.is_none() {
            return Vec::new();
        }
        let mut tabs = vec![Tab::All, Tab::Library];
        tabs.extend(
            self.sections
                .iter()
                .filter(|s| s.source != LIBRARY && !s.tab_groups().is_empty())
                .map(|s| Tab::Source(s.source.clone())),
        );
        tabs
    }

    /// The tab shown: the one chosen while it is still offered, else All.
    pub fn tab(&self) -> Tab {
        if self.tabs().contains(&self.tab) {
            self.tab.clone()
        } else {
            Tab::All
        }
    }

    pub fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
    }

    /// From Your Sources on the All tab: the sources whose terms allow
    /// merging and that found something or failed, in the order they
    /// answered.
    pub fn merged_sources(&self) -> Vec<&Section> {
        self.sections
            .iter()
            .filter(|s| s.source != LIBRARY && s.merged && (!s.items.is_empty() || s.error.is_some()))
            .collect()
    }

    /// The sources whose terms forbid merging and that found something: All
    /// links to their tabs ("Results from Spotify").
    pub fn separate_sources(&self) -> Vec<&Section> {
        self.sections
            .iter()
            .filter(|s| s.source != LIBRARY && !s.merged && !s.tab_groups().is_empty())
            .collect()
    }

    /// True once the last source has answered.
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// True while waiting for the first answer.
    pub fn is_waiting(&self) -> bool {
        self.request.is_some() && !self.done
    }

    /// True when every source answered and none found anything.
    pub fn is_empty_result(&self) -> bool {
        self.done
            && self
                .sections
                .iter()
                .all(|s| s.items.is_empty() && s.tab_groups().is_empty())
    }

    // ---- Show More ---------------------------------------------------------------------------

    /// The cursor Show More sends for one kind of a source's results; `None`
    /// when there are no more, or while more are loading.
    pub fn more_cursor(&self, source: &str, kind: ItemKind) -> Option<String> {
        if self.is_loading_more(source, kind) {
            return None;
        }
        self.section(source)?
            .groups
            .iter()
            .find(|g| g.kind == kind)?
            .next
            .clone()
    }

    /// Show More was sent for one kind of a source's results, as `request`.
    pub fn more_sent(&mut self, source: &str, kind: ItemKind, request: CommandId) {
        self.more_errors.remove(&(source.to_owned(), kind));
        self.more.insert(request, (source.to_owned(), kind));
    }

    /// True while Show More of a source's kind is loading.
    pub fn is_loading_more(&self, source: &str, kind: ItemKind) -> bool {
        self.more.values().any(|(s, k)| s == source && *k == kind)
    }

    /// Why the last Show More of a source's kind failed.
    pub fn more_error(&self, source: &str, kind: ItemKind) -> Option<&str> {
        self.more_errors
            .get(&(source.to_owned(), kind))
            .map(String::as_str)
    }

    /// A Show More command failed: its group keeps its cursor, so it can be
    /// tried again. False when the command was no Show More of this search.
    pub fn more_failed(&mut self, request: CommandId, error: &str) -> bool {
        let Some(key) = self.more.remove(&request) else {
            return false;
        };
        self.more_errors.insert(key, error.to_owned());
        true
    }

    /// Show More's answer: its rows join their group once, and the group
    /// keeps the new cursor (the old one after a failure that brought
    /// nothing). False when it answers no Show More of this search.
    pub fn apply_page(&mut self, page: &SearchPage) -> bool {
        let Some(key) = self.more.remove(&page.request) else {
            return false;
        };
        let failed = page.error.is_some();
        if let Some(error) = &page.error {
            self.more_errors.insert(key, error.clone());
        }
        let group = self
            .sections
            .iter_mut()
            .find(|s| s.source == page.source)
            .and_then(|s| s.groups.iter_mut().find(|g| g.kind == page.kind));
        if let Some(group) = group {
            let shown: HashSet<String> = group.items.iter().map(|i| i.id.clone()).collect();
            group
                .items
                .extend(page.items.iter().filter(|i| !shown.contains(&i.id)).cloned());
            if !(failed && page.items.is_empty()) {
                group.next = page.next.clone();
            }
        }
        true
    }
}

/// The other sources that have an item (`SourceItem::also_in`, merged by
/// the core), named by `name` and joined with `separator`, each once and in
/// the core's order, for "Also in …"; `None` when no other source has it.
pub fn also_in(ids: &[String], name: impl Fn(&str) -> String, separator: &str) -> Option<String> {
    let mut names: Vec<String> = Vec::new();
    for id in ids {
        let n = name(id);
        if !n.trim().is_empty() && !names.contains(&n) {
            names.push(n);
        }
    }
    (!names.is_empty()).then(|| names.join(separator))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(source: &str, kind: ItemKind, title: &str) -> SourceItem {
        SourceItem {
            source: source.into(),
            kind,
            id: title.into(),
            title: title.into(),
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
        }
    }

    fn group(source: &str, kind: ItemKind, titles: &[&str], next: Option<&str>) -> SearchGroup {
        SearchGroup {
            kind,
            items: titles.iter().map(|t| item(source, kind, t)).collect(),
            next: next.map(String::from),
            total: None,
        }
    }

    fn update(request: u64, source: &str, items: Vec<SourceItem>, done: bool) -> Arc<SearchUpdate> {
        answer(request, source, items, true, vec![], done)
    }

    fn answer(
        request: u64,
        source: &str,
        items: Vec<SourceItem>,
        merged: bool,
        groups: Vec<SearchGroup>,
        done: bool,
    ) -> Arc<SearchUpdate> {
        Arc::new(SearchUpdate {
            request: CommandId(request),
            query: "q".into(),
            source: source.into(),
            items,
            merged,
            groups,
            error: None,
            done,
        })
    }

    fn page(request: u64, source: &str, kind: ItemKind, titles: &[&str], next: Option<&str>) -> SearchPage {
        SearchPage {
            request: CommandId(request),
            query: "q".into(),
            source: source.into(),
            kind,
            items: titles.iter().map(|t| item(source, kind, t)).collect(),
            next: next.map(String::from),
            error: None,
        }
    }

    fn titles(state: &SearchState, source: &str, kind: ItemKind) -> Vec<String> {
        let section = state.section(source).expect("section");
        let group = section.groups.iter().find(|g| g.kind == kind).expect("group");
        group.items.iter().map(|i| i.title.clone()).collect()
    }

    #[test]
    fn keeps_the_latest_request_and_puts_the_library_first() {
        let mut s = SearchState::new();
        s.start("tide", CommandId(2));
        assert!(s.is_waiting());
        assert!(!s.apply(&update(1, LIBRARY, vec![], true)), "older request");
        assert!(s.apply(&update(
            2,
            "org.example.subsonic",
            vec![item("s", ItemKind::Track, "a")],
            false
        )));
        assert!(s.apply(&update(
            2,
            LIBRARY,
            vec![item(LIBRARY, ItemKind::Album, "b")],
            true
        )));
        let sources: Vec<&str> = s.sections().iter().map(|x| x.source.as_str()).collect();
        assert_eq!(sources, [LIBRARY, "org.example.subsonic"]);
        assert!(s.is_done());
        assert!(!s.is_waiting());
        assert!(!s.is_empty_result());
        assert_eq!(s.query(), "tide");
    }

    #[test]
    fn a_repeated_source_replaces_its_section() {
        let mut s = SearchState::new();
        s.start("x", CommandId(5));
        s.apply(&update(5, "p", vec![item("p", ItemKind::Track, "1")], false));
        s.apply(&update(5, "p", vec![], true));
        assert_eq!(s.sections().len(), 1);
        assert!(s.is_empty_result());
        s.clear();
        assert!(s.sections().is_empty());
        assert!(!s.is_waiting());
        assert!(s.tabs().is_empty());
    }

    #[test]
    fn names_the_other_sources_once_in_order() {
        let name = |id: &str| match id {
            "org.example.navidrome" => "Navidrome".to_owned(),
            "org.example.jellyfin" | "org.example.jellyfin2" => "Jellyfin".to_owned(),
            other => other.to_owned(),
        };
        let ids: Vec<String> = [
            "org.example.navidrome",
            "org.example.jellyfin",
            "org.example.jellyfin2",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(also_in(&ids, name, ", ").as_deref(), Some("Navidrome, Jellyfin"));
        assert_eq!(also_in(&ids[1..2], name, "、").as_deref(), Some("Jellyfin"));
        assert_eq!(also_in(&[], name, ", "), None);
        assert_eq!(also_in(&[String::new()], name, ", "), None);
    }

    #[test]
    fn groups_by_kind() {
        let section = Section {
            source: LIBRARY.into(),
            items: vec![
                item(LIBRARY, ItemKind::Artist, "c"),
                item(LIBRARY, ItemKind::Track, "a"),
                item(LIBRARY, ItemKind::Track, "b"),
            ],
            error: None,
            merged: true,
            groups: vec![],
        };
        assert_eq!(section.kinds(), vec![ItemKind::Track, ItemKind::Artist]);
        // Without groups of its own, its tab shows its rows kind by kind.
        let tab: Vec<(ItemKind, usize, Option<String>)> = section
            .tab_groups()
            .into_iter()
            .map(|g| (g.kind, g.items.len(), g.next))
            .collect();
        assert_eq!(tab, [(ItemKind::Track, 2, None), (ItemKind::Artist, 1, None)]);
    }

    #[test]
    fn tabs_all_library_then_each_source_that_found_something() {
        let mut s = SearchState::new();
        s.start("tide", CommandId(7));
        s.apply(&update(7, LIBRARY, vec![], false));
        // Merged, with its own groups.
        s.apply(&answer(
            7,
            "nav",
            vec![item("nav", ItemKind::Track, "a")],
            true,
            vec![group("nav", ItemKind::Track, &["a", "b"], Some("c1"))],
            false,
        ));
        // Kept apart by its terms.
        s.apply(&answer(
            7,
            "spotify",
            vec![item("spotify", ItemKind::Track, "x")],
            false,
            vec![
                group("spotify", ItemKind::Track, &["x"], None),
                group("spotify", ItemKind::Album, &[], None),
            ],
            false,
        ));
        // Failed: shown under From Your Sources, but no tab.
        let mut failed = (*answer(7, "jelly", vec![], true, vec![], true)).clone();
        failed.error = Some("Timed out".into());
        s.apply(&Arc::new(failed));

        assert_eq!(
            s.tabs(),
            [
                Tab::All,
                Tab::Library,
                Tab::Source("nav".into()),
                Tab::Source("spotify".into())
            ]
        );
        let merged: Vec<&str> = s.merged_sources().iter().map(|x| x.source.as_str()).collect();
        assert_eq!(merged, ["nav", "jelly"]);
        let separate: Vec<&str> = s.separate_sources().iter().map(|x| x.source.as_str()).collect();
        assert_eq!(separate, ["spotify"]);
        // A source's tab leaves out its kinds that found nothing.
        let kinds: Vec<ItemKind> = s
            .section("spotify")
            .unwrap()
            .tab_groups()
            .iter()
            .map(|g| g.kind)
            .collect();
        assert_eq!(kinds, [ItemKind::Track]);
        assert!(!s.is_empty_result());
    }

    #[test]
    fn a_new_search_starts_on_all() {
        let mut s = SearchState::new();
        s.start("a", CommandId(1));
        s.apply(&answer(
            1,
            "nav",
            vec![],
            true,
            vec![group("nav", ItemKind::Track, &["a"], None)],
            true,
        ));
        s.set_tab(Tab::Source("nav".into()));
        assert_eq!(s.tab(), Tab::Source("nav".into()));
        // A tab no longer offered shows All.
        s.set_tab(Tab::Source("gone".into()));
        assert_eq!(s.tab(), Tab::All);
        s.set_tab(Tab::Library);
        s.start("b", CommandId(2));
        assert_eq!(s.tab(), Tab::All);
        assert_eq!(s.tabs(), [Tab::All, Tab::Library]);
    }

    #[test]
    fn show_more_adds_rows_once_and_keeps_the_new_cursor() {
        let mut s = SearchState::new();
        s.start("tide", CommandId(3));
        s.apply(&answer(
            3,
            "nav",
            vec![],
            true,
            vec![group("nav", ItemKind::Track, &["1", "2"], Some("c1"))],
            true,
        ));
        assert_eq!(s.more_cursor("nav", ItemKind::Track).as_deref(), Some("c1"));
        assert_eq!(s.more_cursor("nav", ItemKind::Album), None);
        s.more_sent("nav", ItemKind::Track, CommandId(9));
        assert!(s.is_loading_more("nav", ItemKind::Track));
        assert_eq!(s.more_cursor("nav", ItemKind::Track), None, "one at a time");
        assert!(
            !s.apply_page(&page(8, "nav", ItemKind::Track, &["x"], None)),
            "not ours"
        );
        assert!(s.apply_page(&page(9, "nav", ItemKind::Track, &["2", "3"], Some("c2"))));
        assert_eq!(titles(&s, "nav", ItemKind::Track), ["1", "2", "3"]);
        assert!(!s.is_loading_more("nav", ItemKind::Track));
        assert_eq!(s.more_cursor("nav", ItemKind::Track).as_deref(), Some("c2"));
        // The last page has no cursor: no more Show More.
        s.more_sent("nav", ItemKind::Track, CommandId(10));
        s.apply_page(&page(10, "nav", ItemKind::Track, &["4"], None));
        assert_eq!(s.more_cursor("nav", ItemKind::Track), None);
        // A new search forgets Show More in flight.
        s.more_sent("nav", ItemKind::Track, CommandId(11));
        s.start("other", CommandId(12));
        assert!(!s.apply_page(&page(11, "nav", ItemKind::Track, &["5"], None)));
    }

    #[test]
    fn a_failed_show_more_says_why_and_can_be_tried_again() {
        let mut s = SearchState::new();
        s.start("tide", CommandId(3));
        s.apply(&answer(
            3,
            "nav",
            vec![],
            true,
            vec![group("nav", ItemKind::Album, &["1"], Some("c1"))],
            true,
        ));
        s.more_sent("nav", ItemKind::Album, CommandId(4));
        assert!(!s.more_failed(CommandId(99), "nope"), "not ours");
        assert!(s.more_failed(CommandId(4), "The server didn't answer."));
        assert_eq!(
            s.more_error("nav", ItemKind::Album),
            Some("The server didn't answer.")
        );
        assert_eq!(s.more_cursor("nav", ItemKind::Album).as_deref(), Some("c1"));
        // Trying again clears the reason; a page that failed keeps the cursor.
        s.more_sent("nav", ItemKind::Album, CommandId(5));
        assert_eq!(s.more_error("nav", ItemKind::Album), None);
        let mut failed = page(5, "nav", ItemKind::Album, &[], None);
        failed.error = Some("Timed out".into());
        assert!(s.apply_page(&failed));
        assert_eq!(s.more_error("nav", ItemKind::Album), Some("Timed out"));
        assert_eq!(s.more_cursor("nav", ItemKind::Album).as_deref(), Some("c1"));
    }
}
