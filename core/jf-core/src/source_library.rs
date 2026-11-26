//! Sources' likes and playlists (ADR-0024 §2): the lists behind the
//! `SourceLibrary` slice. They are loaded when the apps ask
//! (`RefreshSourceLibrary`, never polled) and kept on disk, so Favourites
//! and Playlists show the last lists at once, offline too, with their age.
//!
//! Each list keeps its first page (at most [`FIRST_PAGE`] entries) as the
//! plugin gave it, in `<data_dir>/source-library/<plugin>.json`; more pages
//! (`SourceLibraryMore`) go to the app only. A list the source says it does
//! not keep (`unsupported`: liked artists, say) is left out from then on.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use jf_plugin_host::Item;
use serde::{Deserialize, Serialize};

use crate::types::{SourceItem, SourceLibraryEntry, SourceLibraryList, SourceLibraryPart};

/// The entries of a list kept and published: its first page.
pub(crate) const FIRST_PAGE: u32 = 100;

/// The lists, in the slice's order.
pub(crate) const PARTS: [SourceLibraryPart; 4] = [
    SourceLibraryPart::LikedSongs,
    SourceLibraryPart::LikedAlbums,
    SourceLibraryPart::LikedArtists,
    SourceLibraryPart::Playlists,
];

/// An entry as the plugin gave it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub item: Item,
    /// Playlists: the person's own (or one they collaborate on).
    pub owned: bool,
    /// Playlists: the source lists its songs here.
    pub songs_listed: bool,
}

/// A page of a list, as the plugin answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Page {
    pub entries: Vec<Entry>,
    pub next: Option<String>,
    pub total: Option<u32>,
}

/// Why a list could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Failure {
    /// The source does not keep this list.
    Unsupported,
    Error(String),
}

/// One list of a source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct List {
    part: SourceLibraryPart,
    entries: Vec<Entry>,
    total: Option<u32>,
    next: Option<String>,
    refreshed_at_ms: Option<i64>,
    #[serde(skip)]
    loading: bool,
    #[serde(skip)]
    error: Option<String>,
}

impl List {
    fn empty(part: SourceLibraryPart) -> List {
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

/// One source's lists, as kept on disk.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Source {
    lists: Vec<List>,
    /// The lists it said it does not keep.
    #[serde(default)]
    unsupported: Vec<SourceLibraryPart>,
}

impl Source {
    fn list(&mut self, part: SourceLibraryPart) -> &mut List {
        let at = match self.lists.iter().position(|l| l.part == part) {
            Some(at) => at,
            None => {
                self.lists.push(List::empty(part));
                self.lists.len() - 1
            }
        };
        &mut self.lists[at]
    }
}

/// The lists of the sources with the `library` capability.
#[derive(Debug)]
pub(crate) struct Lists {
    /// `<data_dir>/source-library`.
    dir: PathBuf,
    /// Read from disk on first use.
    sources: HashMap<String, Source>,
}

impl Lists {
    pub(crate) fn new(dir: PathBuf) -> Lists {
        Lists {
            dir,
            sources: HashMap::new(),
        }
    }

    fn path(&self, plugin: &str) -> PathBuf {
        self.dir.join(format!("{plugin}.json"))
    }

    /// A source's lists, read from disk the first time.
    fn source(&mut self, plugin: &str) -> &mut Source {
        let path = self.path(plugin);
        self.sources
            .entry(plugin.to_owned())
            .or_insert_with(|| read(&path).unwrap_or_default())
    }

    /// A refresh of `plugin` began: its lists show as loading.
    pub(crate) fn loading(&mut self, plugin: &str) {
        let source = self.source(plugin);
        for part in PARTS {
            if !source.unsupported.contains(&part) {
                source.list(part).loading = true;
            }
        }
    }

    /// A list's first page arrived, or why it did not. What came is kept on
    /// disk; on a failure the last lists stay, with the error.
    pub(crate) fn loaded(
        &mut self,
        plugin: &str,
        part: SourceLibraryPart,
        result: Result<Page, Failure>,
        now_ms: i64,
    ) {
        let path = self.path(plugin);
        let source = self.source(plugin);
        match result {
            Ok(page) => {
                source.unsupported.retain(|p| *p != part);
                let list = source.list(part);
                list.entries = page.entries;
                list.entries.truncate(FIRST_PAGE as usize);
                list.total = page.total;
                list.next = page.next;
                list.refreshed_at_ms = Some(now_ms);
                list.loading = false;
                list.error = None;
            }
            Err(Failure::Unsupported) => {
                source.lists.retain(|l| l.part != part);
                if !source.unsupported.contains(&part) {
                    source.unsupported.push(part);
                }
            }
            Err(Failure::Error(message)) => {
                let list = source.list(part);
                list.loading = false;
                list.error = Some(message);
                return;
            }
        }
        if let Err(e) = write(&path, source) {
            tracing::warn!(%plugin, "could not keep the source's likes and playlists: {e}");
        }
    }

    /// A source's lists for the slice, in [`PARTS`] order, with each item made
    /// a row by `row`. Lists not loaded yet are empty, without an age.
    pub(crate) fn lists(
        &mut self,
        plugin: &str,
        row: impl Fn(&Item) -> SourceItem,
    ) -> Vec<SourceLibraryList> {
        let source = self.source(plugin);
        PARTS
            .iter()
            .filter(|part| !source.unsupported.contains(part))
            .map(|part| {
                let list = source
                    .lists
                    .iter()
                    .find(|l| l.part == *part)
                    .cloned()
                    .unwrap_or_else(|| List::empty(*part));
                SourceLibraryList {
                    part: *part,
                    entries: list
                        .entries
                        .iter()
                        .map(|e| SourceLibraryEntry {
                            item: row(&e.item),
                            owned: e.owned,
                            songs_listed: e.songs_listed,
                        })
                        .collect(),
                    total: list.total,
                    next: list.next,
                    refreshed_at_ms: list.refreshed_at_ms,
                    loading: list.loading,
                    error: list.error,
                }
            })
            .collect()
    }

    /// Forgets a source's lists, on disk too (it signed out).
    pub(crate) fn forget(&mut self, plugin: &str) {
        self.sources.remove(plugin);
        let path = self.path(plugin);
        if let Err(e) = std::fs::remove_file(&path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(%plugin, "could not delete the source's kept likes and playlists: {e}");
        }
    }

    /// The sources whose lists are kept on disk.
    pub(crate) fn kept(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                name.strip_suffix(".json").map(str::to_owned)
            })
            .collect()
    }
}

fn read(path: &Path) -> Option<Source> {
    let bytes = std::fs::read(path).ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(source) => Some(source),
        Err(e) => {
            tracing::warn!(path = %path.display(), "ignoring unreadable kept likes and playlists: {e}");
            None
        }
    }
}

fn write(path: &Path, source: &Source) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(source).map_err(std::io::Error::other)?;
    crate::sources::write_atomic(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_plugin_host::ItemKind;

    fn entry(id: &str, kind: ItemKind) -> Entry {
        Entry {
            item: crate::sources::bare_item(id, kind),
            owned: true,
            songs_listed: true,
        }
    }

    fn row(item: &Item) -> SourceItem {
        crate::sources::source_item("org.example.likes", item, &Default::default())
    }

    fn page(entries: Vec<Entry>, next: Option<&str>) -> Page {
        Page {
            total: Some(entries.len() as u32 + u32::from(next.is_some())),
            entries,
            next: next.map(str::to_owned),
        }
    }

    #[test]
    fn lists_load_keep_their_age_and_stay_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let mut lists = Lists::new(dir.path().to_path_buf());
        let plugin = "org.example.likes";
        let shown = lists.lists(plugin, row);
        assert_eq!(shown.len(), 4, "every list until the source says otherwise");
        assert!(
            shown
                .iter()
                .all(|l| l.entries.is_empty() && l.refreshed_at_ms.is_none() && !l.loading)
        );

        lists.loading(plugin);
        assert!(lists.lists(plugin, row).iter().all(|l| l.loading));
        lists.loaded(
            plugin,
            SourceLibraryPart::LikedSongs,
            Ok(page(
                vec![entry("t1", ItemKind::Track), entry("t2", ItemKind::Track)],
                Some("2"),
            )),
            1_000,
        );
        lists.loaded(
            plugin,
            SourceLibraryPart::LikedArtists,
            Err(Failure::Unsupported),
            1_000,
        );
        lists.loaded(
            plugin,
            SourceLibraryPart::Playlists,
            Err(Failure::Error("the server is down".into())),
            1_000,
        );
        let shown = lists.lists(plugin, row);
        let parts: Vec<_> = shown.iter().map(|l| l.part).collect();
        assert_eq!(
            parts,
            [
                SourceLibraryPart::LikedSongs,
                SourceLibraryPart::LikedAlbums,
                SourceLibraryPart::Playlists
            ],
            "liked artists left out"
        );
        let songs = &shown[0];
        assert_eq!(songs.entries.len(), 2);
        assert_eq!((songs.next.as_deref(), songs.total), (Some("2"), Some(3)));
        assert_eq!((songs.refreshed_at_ms, songs.loading), (Some(1_000), false));
        assert!(shown[1].loading, "still loading");
        assert_eq!(shown[2].error.as_deref(), Some("the server is down"));

        // Another run reads them back, with their age and without the error.
        let mut again = Lists::new(dir.path().to_path_buf());
        let shown = again.lists(plugin, row);
        assert_eq!(shown.len(), 3);
        assert_eq!(shown[0].entries.len(), 2);
        assert_eq!(shown[0].refreshed_at_ms, Some(1_000));
        assert!(shown.iter().all(|l| !l.loading && l.error.is_none()));
        assert_eq!(again.kept(), [plugin]);

        again.forget(plugin);
        assert!(again.kept().is_empty());
        assert!(again.lists(plugin, row).iter().all(|l| l.entries.is_empty()));
    }
}
