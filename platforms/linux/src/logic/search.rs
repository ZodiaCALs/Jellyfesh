//! Unified search as it streams in: one section per source, in the order the
//! sources answer, the library first.
//!
//! The core merges and de-duplicates across sources (`SourceItem::also_in`);
//! this only keeps the latest request's answers and groups each source's
//! items by kind for display.

use std::sync::Arc;

use jf_core::{CommandId, ItemKind, SearchUpdate, SourceItem};

/// The source id the core uses for the local library.
pub const LIBRARY: &str = "library";

/// One source's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub source: String,
    pub items: Vec<SourceItem>,
    pub error: Option<String>,
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
}

/// The search page's state.
#[derive(Debug, Default)]
pub struct SearchState {
    query: String,
    request: Option<CommandId>,
    sections: Vec<Section>,
    done: bool,
}

impl SearchState {
    pub fn new() -> Self {
        Self::default()
    }

    /// A new search was sent: earlier answers no longer apply.
    pub fn start(&mut self, query: &str, request: CommandId) {
        self.query = query.to_owned();
        self.request = Some(request);
        self.sections.clear();
        self.done = false;
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

    pub fn sections(&self) -> &[Section] {
        &self.sections
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
        self.done && self.sections.iter().all(|s| s.items.is_empty())
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
        }
    }

    fn update(request: u64, source: &str, items: Vec<SourceItem>, done: bool) -> Arc<SearchUpdate> {
        Arc::new(SearchUpdate {
            request: CommandId(request),
            query: "q".into(),
            source: source.into(),
            items,
            error: None,
            done,
        })
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
    }

    #[test]
    fn names_the_other_sources_once_in_order() {
        let name = |id: &str| match id {
            "org.example.navidrome" => "Navidrome".to_owned(),
            "org.example.jellyfin" | "org.example.jellyfin2" => "Jellyfin".to_owned(),
            other => other.to_owned(),
        };
        let ids: Vec<String> = ["org.example.navidrome", "org.example.jellyfin", "org.example.jellyfin2"]
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
        };
        assert_eq!(section.kinds(), vec![ItemKind::Track, ItemKind::Artist]);
    }
}
