//! What an item's main action and menus offer around links and remote
//! sources (ADR-0018; docs/ui.md, "Links and remote playback"): an item
//! with a `link` offers Open in {Service}; one that cannot play here opens
//! its link as its main action, next to its unplayable reason; a remote
//! source's songs never join the queue or a playlist. The core fills in the
//! links and says which sources are remote; this only reads what it says.

use jf_core::{ItemKind, SourceItem};

/// What clicking an item (or pressing Enter on it) does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainAction {
    /// Plays it (a track that can play here).
    Play,
    /// Opens its page (an album, artist, playlist or folder).
    Browse,
    /// Opens it in its service: it cannot play here, but has a link.
    OpenLink,
    /// Nothing: it cannot play here and has no link.
    Nothing,
}

/// The main action of a search or browse result.
pub fn main_action(item: &SourceItem) -> MainAction {
    let can_play = item.playable;
    let has_link = item.link.is_some();
    match item.kind {
        ItemKind::Track if can_play && item.track.is_some() => MainAction::Play,
        _ if !can_play && has_link => MainAction::OpenLink,
        ItemKind::Track => MainAction::Nothing,
        ItemKind::Album | ItemKind::Artist | ItemKind::Playlist | ItemKind::Folder => MainAction::Browse,
    }
}

/// Whether Play Next, Play Last and Add to Playlist are offered for an
/// item: never for a remote source's songs (they play on the person's own
/// devices and never join the queue), nor for items that cannot play here.
pub fn offers_queueing(item: &SourceItem, remote_source: bool) -> bool {
    !remote_source && item.playable
}

/// The position to start from in a list of rows when row `clicked` is
/// played: rows before it that cannot play are not in the queued list.
pub fn start_index(playable: &[bool], clicked: usize) -> usize {
    playable.iter().take(clicked).filter(|p| **p).count()
}

/// Whether a link may be opened: Open in {Service} is for `https` links
/// (the plugin host checks the templates; the shell opens nothing else).
pub fn openable(link: &str) -> bool {
    link.len() > "https://".len() && link.get(..8).is_some_and(|s| s.eq_ignore_ascii_case("https://"))
}

#[cfg(test)]
mod tests {
    use jf_core::{SourceTrack, TrackRef};

    use super::*;

    fn item(kind: ItemKind, playable: bool, track: bool, link: bool) -> SourceItem {
        SourceItem {
            source: "org.example.music".into(),
            kind,
            id: "42".into(),
            title: "Sample".into(),
            subtitle: None,
            album: None,
            duration_ms: None,
            artwork: None,
            playable,
            unplayable_reason: (!playable).then(|| "Not available in your region".into()),
            track: track.then(|| {
                TrackRef::Source(SourceTrack {
                    plugin: "org.example.music".into(),
                    id: "42".into(),
                    title: "Sample".into(),
                    artist: None,
                    album: None,
                    duration_ms: None,
                    artwork_url: None,
                    isrc: None,
                })
            }),
            also_in: Vec::new(),
            link: link.then(|| "https://music.example.com/song/42".into()),
        }
    }

    #[test]
    fn playable_tracks_play() {
        assert_eq!(
            main_action(&item(ItemKind::Track, true, true, true)),
            MainAction::Play
        );
        assert_eq!(
            main_action(&item(ItemKind::Track, true, true, false)),
            MainAction::Play
        );
    }

    #[test]
    fn items_that_cannot_play_open_their_link() {
        assert_eq!(
            main_action(&item(ItemKind::Track, false, false, true)),
            MainAction::OpenLink
        );
        assert_eq!(
            main_action(&item(ItemKind::Album, false, false, true)),
            MainAction::OpenLink
        );
        // Without a link they do nothing, and show as unavailable.
        assert_eq!(
            main_action(&item(ItemKind::Track, false, false, false)),
            MainAction::Nothing
        );
    }

    #[test]
    fn containers_open_their_page() {
        for kind in [
            ItemKind::Album,
            ItemKind::Artist,
            ItemKind::Playlist,
            ItemKind::Folder,
        ] {
            assert_eq!(main_action(&item(kind, true, false, true)), MainAction::Browse);
        }
        // A container that cannot play and has no link is still browsed.
        assert_eq!(
            main_action(&item(ItemKind::Album, false, false, false)),
            MainAction::Browse
        );
    }

    #[test]
    fn remote_songs_never_queue() {
        let song = item(ItemKind::Track, true, true, true);
        assert!(offers_queueing(&song, false));
        assert!(!offers_queueing(&song, true));
        assert!(!offers_queueing(
            &item(ItemKind::Track, false, false, true),
            false
        ));
    }

    #[test]
    fn the_start_skips_rows_that_cannot_play() {
        let playable = [true, false, true, true];
        assert_eq!(start_index(&playable, 0), 0);
        assert_eq!(start_index(&playable, 2), 1);
        assert_eq!(start_index(&playable, 3), 2);
        assert_eq!(start_index(&playable, 9), 3);
    }

    #[test]
    fn only_https_links_open() {
        assert!(openable("https://music.example.com/song/42"));
        assert!(openable("HTTPS://music.example.com/"));
        assert!(!openable("https://"));
        assert!(!openable("http://music.example.com/song/42"));
        assert!(!openable("file:///etc/passwd"));
        assert!(!openable("javascript:alert(1)"));
    }
}
