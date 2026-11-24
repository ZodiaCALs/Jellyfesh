//! The sources' likes and playlists (ADR-0024 §2), kept as the core's
//! `SourceLibrary` slice keeps them, for Favourites and Playlists:
//!
//! * one entry per enabled, signed-in source with the `library` capability,
//!   in the order the page lists its sources;
//! * its lists in the order Liked Songs, Liked Albums, Liked Artists,
//!   Playlists; a list the source does not keep (it answered `unsupported`)
//!   is left out;
//! * each list holds its first page ([`FIRST_PAGE`]), `total`, the cursor of
//!   its next page, when it was refreshed, whether a refresh runs and why the
//!   last one failed (the entries are then the cached ones);
//! * the lists refresh when Favourites or Playlists opens, or is shown again
//!   after [`STALE_MS`] ([`SourceLibraries::stale`]); there is no polling;
//! * the page keeps the lists (IndexedDB) so they show at once, offline too,
//!   with their age.

use jf_plugin_model::{Item, ItemKind, ItemPage, PlaylistPage};
use serde::{Deserialize, Serialize};

/// The first page of each list.
pub const FIRST_PAGE: u32 = 100;
/// A list shown again this long after its refresh refreshes again.
pub const STALE_MS: i64 = 10 * 60 * 1000;

/// One of a source's lists (the core's `SourceLibraryPart`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Part {
    LikedSongs,
    LikedAlbums,
    LikedArtists,
    Playlists,
}

impl Part {
    pub const ALL: [Part; 4] = [
        Part::LikedSongs,
        Part::LikedAlbums,
        Part::LikedArtists,
        Part::Playlists,
    ];

    /// The kind `library.likes` is asked for, or `None` for Playlists.
    pub fn liked_kind(self) -> Option<ItemKind> {
        match self {
            Part::LikedSongs => Some(ItemKind::Track),
            Part::LikedAlbums => Some(ItemKind::Album),
            Part::LikedArtists => Some(ItemKind::Artist),
            Part::Playlists => None,
        }
    }
}

/// An entry of a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub item: Item,
    /// Playlists: the person's own (or one they collaborate on).
    pub owned: bool,
    /// Playlists: the source lists its songs here; true for likes.
    pub songs_listed: bool,
}

/// One list of a source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct List {
    pub part: Part,
    pub entries: Vec<Entry>,
    pub total: Option<u32>,
    pub next: Option<String>,
    pub refreshed_at_ms: Option<i64>,
    pub loading: bool,
    pub error: Option<String>,
}

impl List {
    fn new(part: Part) -> List {
        List {
            part,
            entries: Vec::new(),
            total: None,
            next: None,
            refreshed_at_ms: None,
            loading: false,
            error: None,
        }
    }
}

/// A source's likes and playlists (the core's `SourceLibrary`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLibrary {
    pub plugin: String,
    pub name: String,
    /// What its views must say it comes from ("Content from Spotify").
    pub attribution: Option<String>,
    pub lists: Vec<List>,
    /// The lists it answered `unsupported` for.
    #[serde(default)]
    unsupported: Vec<Part>,
}

/// A source the slice covers.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Covered {
    pub plugin: String,
    pub name: String,
    pub attribution: Option<String>,
}

/// Every covered source's lists. See the module documentation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLibraries {
    sources: Vec<SourceLibrary>,
}

fn entries_of_likes(page: ItemPage) -> Vec<Entry> {
    page.items
        .into_iter()
        .map(|item| Entry {
            item,
            owned: false,
            songs_listed: true,
        })
        .collect()
}

fn entries_of_playlists(page: &PlaylistPage) -> Vec<Entry> {
    page.entries
        .iter()
        .map(|e| Entry {
            item: e.item.clone(),
            owned: e.owned,
            songs_listed: e.songs_listed,
        })
        .collect()
}

impl SourceLibraries {
    /// The lists the page kept, or none when they cannot be read. Refreshes
    /// that were running when the page closed are over.
    pub fn from_cache(text: &str) -> SourceLibraries {
        let mut loaded: SourceLibraries = serde_json::from_str(text).unwrap_or_default();
        for list in loaded.sources.iter_mut().flat_map(|s| s.lists.iter_mut()) {
            list.loading = false;
        }
        loaded
    }

    /// What the page keeps.
    pub fn cache(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }

    /// The covered sources now: others are forgotten, new ones start
    /// empty, and the order follows `covered`.
    pub fn cover(&mut self, covered: Vec<Covered>) {
        let mut old = std::mem::take(&mut self.sources);
        self.sources = covered
            .into_iter()
            .map(|c| match old.iter().position(|s| s.plugin == c.plugin) {
                Some(i) => {
                    let mut kept = old.swap_remove(i);
                    kept.name = c.name;
                    kept.attribution = c.attribution;
                    kept
                }
                None => SourceLibrary {
                    plugin: c.plugin,
                    name: c.name,
                    attribution: c.attribution,
                    lists: Vec::new(),
                    unsupported: Vec::new(),
                },
            })
            .collect();
    }

    /// The lists to refresh for `plugin` at `now_ms`: every list it may keep
    /// when the page `opened`; when it is shown again, those refreshed more
    /// than [`STALE_MS`] ago (or never).
    pub fn stale(&self, plugin: &str, now_ms: i64, opened: bool) -> Vec<Part> {
        let Some(source) = self.source(plugin) else {
            return Vec::new();
        };
        Part::ALL
            .into_iter()
            .filter(|part| !source.unsupported.contains(part))
            .filter(|part| {
                let list = source.lists.iter().find(|l| l.part == *part);
                if list.is_some_and(|l| l.loading) {
                    return false;
                }
                opened
                    || list
                        .and_then(|l| l.refreshed_at_ms)
                        .is_none_or(|at| now_ms - at > STALE_MS)
            })
            .collect()
    }

    /// A refresh of `part` began.
    pub fn begin(&mut self, plugin: &str, part: Part) {
        if let Some(list) = self.list_mut(plugin, part) {
            list.loading = true;
        }
    }

    /// The first page of liked items arrived.
    pub fn likes(&mut self, plugin: &str, part: Part, page: ItemPage, now_ms: i64) {
        let (next, total) = (page.next.clone(), page.total);
        self.refreshed(plugin, part, entries_of_likes(page), next, total, now_ms);
    }

    /// The first page of playlists arrived.
    pub fn playlists(&mut self, plugin: &str, page: PlaylistPage, now_ms: i64) {
        let entries = entries_of_playlists(&page);
        self.refreshed(plugin, Part::Playlists, entries, page.next, page.total, now_ms);
    }

    fn refreshed(
        &mut self,
        plugin: &str,
        part: Part,
        entries: Vec<Entry>,
        next: Option<String>,
        total: Option<u32>,
        now_ms: i64,
    ) {
        if let Some(list) = self.list_mut(plugin, part) {
            list.entries = entries;
            list.next = next;
            list.total = total;
            list.refreshed_at_ms = Some(now_ms);
            list.loading = false;
            list.error = None;
        }
    }

    /// The source does not keep `part`: the list goes, and is not asked for
    /// again.
    pub fn unsupported(&mut self, plugin: &str, part: Part) {
        if let Some(source) = self.source_mut(plugin) {
            source.lists.retain(|l| l.part != part);
            if !source.unsupported.contains(&part) {
                source.unsupported.push(part);
            }
        }
    }

    /// A refresh failed: the cached entries stay, with why.
    pub fn fail(&mut self, plugin: &str, part: Part, error: &str) {
        if let Some(list) = self.list_mut(plugin, part) {
            list.loading = false;
            list.error = Some(error.to_owned());
        }
    }

    /// The cursor for more of a list, or `None`.
    pub fn next(&self, plugin: &str, part: Part) -> Option<String> {
        self.source(plugin)?
            .lists
            .iter()
            .find(|l| l.part == part)?
            .next
            .clone()
    }

    /// More of a list of likes (`SourceLibraryMore`): added, with the next cursor.
    pub fn more_likes(&mut self, plugin: &str, part: Part, page: ItemPage) {
        let next = page.next.clone();
        self.more(plugin, part, entries_of_likes(page), next);
    }

    /// More playlists.
    pub fn more_playlists(&mut self, plugin: &str, page: PlaylistPage) {
        let entries = entries_of_playlists(&page);
        self.more(plugin, Part::Playlists, entries, page.next);
    }

    fn more(&mut self, plugin: &str, part: Part, entries: Vec<Entry>, next: Option<String>) {
        if let Some(list) = self.list_mut(plugin, part) {
            for entry in entries {
                if !list.entries.iter().any(|e| e.item.id == entry.item.id) {
                    list.entries.push(entry);
                }
            }
            list.next = next;
        }
    }

    /// Every covered source with its lists, in order.
    pub fn sources(&self) -> &[SourceLibrary] {
        &self.sources
    }

    fn source(&self, plugin: &str) -> Option<&SourceLibrary> {
        self.sources.iter().find(|s| s.plugin == plugin)
    }

    fn source_mut(&mut self, plugin: &str) -> Option<&mut SourceLibrary> {
        self.sources.iter_mut().find(|s| s.plugin == plugin)
    }

    /// The list, made in its place when it is new (never for a part the
    /// source does not keep).
    fn list_mut(&mut self, plugin: &str, part: Part) -> Option<&mut List> {
        let source = self.source_mut(plugin)?;
        if source.unsupported.contains(&part) {
            return None;
        }
        if !source.lists.iter().any(|l| l.part == part) {
            let at = source
                .lists
                .iter()
                .position(|l| {
                    Part::ALL.iter().position(|p| *p == l.part) > Part::ALL.iter().position(|p| *p == part)
                })
                .unwrap_or(source.lists.len());
            source.lists.insert(at, List::new(part));
        }
        source.lists.iter_mut().find(|l| l.part == part)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_plugin_model::PlaylistEntry;

    fn item(kind: ItemKind, id: &str) -> Item {
        Item {
            id: id.into(),
            kind,
            title: format!("{kind:?} {id}"),
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
            playable: true,
            unplayable: None,
        }
    }

    fn covered(plugin: &str) -> Covered {
        Covered {
            plugin: plugin.into(),
            name: plugin.to_uppercase(),
            attribution: None,
        }
    }

    fn page(ids: &[&str], next: Option<&str>) -> ItemPage {
        ItemPage {
            items: ids.iter().map(|id| item(ItemKind::Track, id)).collect(),
            next: next.map(str::to_owned),
            total: Some(42),
        }
    }

    #[test]
    fn lists_refresh_on_opening_and_when_stale() {
        let mut libraries = SourceLibraries::default();
        libraries.cover(vec![covered("a")]);
        assert_eq!(libraries.stale("a", 0, false), Part::ALL, "never refreshed");
        assert_eq!(libraries.stale("nobody", 0, true), []);
        for part in Part::ALL {
            libraries.begin("a", part);
        }
        assert_eq!(libraries.stale("a", 0, true), [], "already running");
        libraries.likes("a", Part::LikedSongs, page(&["1", "2"], Some("c2")), 1_000);
        libraries.unsupported("a", Part::LikedArtists);
        libraries.fail("a", Part::LikedAlbums, "offline");
        libraries.playlists(
            "a",
            PlaylistPage {
                entries: vec![PlaylistEntry {
                    item: item(ItemKind::Playlist, "p1"),
                    owned: false,
                    songs_listed: false,
                }],
                next: None,
                total: None,
            },
            1_000,
        );
        let lists = &libraries.sources()[0].lists;
        let parts: Vec<Part> = lists.iter().map(|l| l.part).collect();
        assert_eq!(
            parts,
            [Part::LikedSongs, Part::LikedAlbums, Part::Playlists],
            "in order, without artists"
        );
        assert_eq!(lists[0].entries.len(), 2);
        assert_eq!(lists[0].total, Some(42));
        assert_eq!(lists[1].error.as_deref(), Some("offline"));
        assert!(!lists[1].loading);
        assert!(!lists[2].entries[0].songs_listed);
        // Shown again: only what is older than ten minutes (and never the unsupported list).
        assert_eq!(libraries.stale("a", 1_000 + STALE_MS, false), [Part::LikedAlbums]);
        assert_eq!(
            libraries.stale("a", 2_000 + STALE_MS, false),
            [Part::LikedSongs, Part::LikedAlbums, Part::Playlists]
        );
        assert_eq!(
            libraries.stale("a", 1_500, true).len(),
            3,
            "opening refreshes everything it keeps"
        );
    }

    #[test]
    fn more_pages_add_and_the_cache_comes_back() {
        let mut libraries = SourceLibraries::default();
        libraries.cover(vec![covered("a"), covered("b")]);
        libraries.begin("a", Part::LikedSongs);
        libraries.likes("a", Part::LikedSongs, page(&["1", "2"], Some("c2")), 5);
        assert_eq!(libraries.next("a", Part::LikedSongs).as_deref(), Some("c2"));
        libraries.more_likes("a", Part::LikedSongs, page(&["2", "3"], None));
        let ids: Vec<&str> = libraries.sources()[0].lists[0]
            .entries
            .iter()
            .map(|e| e.item.id.as_str())
            .collect();
        assert_eq!(ids, ["1", "2", "3"]);
        assert_eq!(libraries.next("a", Part::LikedSongs), None);
        libraries.begin("b", Part::Playlists);
        let back = SourceLibraries::from_cache(&libraries.cache());
        assert!(
            !back.sources()[1].lists[0].loading,
            "a refresh does not outlive the page"
        );
        assert_eq!(back.sources()[0], libraries.sources()[0]);
        // A source signed out goes; the order follows the sources.
        let mut moved = back;
        moved.cover(vec![covered("b")]);
        assert_eq!(moved.sources().len(), 1);
        assert_eq!(moved.sources()[0].plugin, "b");
        assert_eq!(
            SourceLibraries::from_cache("nonsense"),
            SourceLibraries::default()
        );
    }
}
