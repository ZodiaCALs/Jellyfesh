//! A test plugin for the API 1.3 exports `library` and `download`, with
//! invented items and words.
//!
//! * `library.likes`: five liked songs (pages follow the host's limit, the
//!   cursor is the next index) and two liked albums; liked artists answer
//!   `unsupported`, as a service that keeps none would.
//! * `library.playlists`: one of the person's own playlists, one they follow
//!   and one they follow whose songs the service lists only in its own app.
//! * `catalog.browse`: each playlist's songs (`pl:followed-unlisted` answers
//!   `not-found`, as Spotify does for such playlists), in pages of two when
//!   the host asks for two.
//! * `download.download`: `{server_url}/files/<id>.<container>` with the
//!   header `x-token: test-token`: the original FLAC, or Opus for the other
//!   qualities. With `refuse` on, the "server" does not let the account
//!   download (`unsupported`).
//!
//! With `hostile` on, every answer breaks the host's limits (pages larger
//! than asked, items of other kinds, overlong texts, a download URL off the
//! allowlist) so the tests can see them checked.

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;

struct Library;

thread_local! {
    static SERVER: RefCell<String> = const { RefCell::new(String::new()) };
    static HOSTILE: RefCell<bool> = const { RefCell::new(false) };
    static REFUSE: RefCell<bool> = const { RefCell::new(false) };
}

fn server() -> String {
    SERVER.with(|s| s.borrow().trim_end_matches('/').to_string())
}

fn hostile() -> bool {
    HOSTILE.with(|h| *h.borrow())
}

fn refuse() -> bool {
    REFUSE.with(|r| *r.borrow())
}

const LIKED_SONGS: usize = 5;

fn song(n: usize) -> Item {
    let mut item = Item::new(ItemKind::Track, format!("tr:{n}"), format!("Invented Song {n}"))
        .artist("Invented Artist")
        .album("Invented Album")
        .duration_ms(1_500);
    item.album_id = Some("al:1".into());
    item.artist_id = Some("ar:1".into());
    item.track_number = Some(u32::try_from(n).unwrap_or(u32::MAX));
    item
}

fn album(n: usize) -> Item {
    Item::new(ItemKind::Album, format!("al:{n}"), format!("Invented Album {n}")).artist("Invented Artist")
}

fn playlist(id: &str, title: &str) -> Item {
    Item::new(ItemKind::Playlist, id, title)
}

/// Items `cursor..` of `all`, at most `limit`, with the cursor of the rest.
fn paged(all: Vec<Item>, page: &PageRequest) -> ItemPage {
    let total = all.len();
    let start = page
        .cursor
        .as_deref()
        .and_then(|c| c.parse::<usize>().ok())
        .unwrap_or(0)
        .min(total);
    let end = (start + page.limit.max(1) as usize).min(total);
    let items = all[start..end].to_vec();
    let mut answer = if end < total {
        ItemPage::with_next(items, end.to_string())
    } else {
        ItemPage::new(items)
    };
    answer.total = u32::try_from(total).ok();
    answer
}

impl Plugin for Library {
    fn capabilities() -> Vec<Capability> {
        vec![Capability::Browse]
    }

    fn settings_changed(settings: &serde_json::Value) -> Result<()> {
        let url = settings["server_url"].as_str().unwrap_or_default().to_string();
        SERVER.with(|s| *s.borrow_mut() = url);
        HOSTILE.with(|h| *h.borrow_mut() = settings["hostile"].as_bool().unwrap_or(false));
        REFUSE.with(|r| *r.borrow_mut() = settings["refuse"].as_bool().unwrap_or(false));
        Ok(())
    }

    fn browse(node: Option<Item>, page: PageRequest) -> Result<ItemPage> {
        match node.as_ref().map(|n| n.id.as_str()) {
            None => Ok(ItemPage::new(vec![Item::new(
                ItemKind::Folder,
                "f:playlists",
                "Playlists",
            )])),
            Some("f:playlists") => Ok(ItemPage::new(vec![
                playlist("pl:mine", "Invented Mix"),
                playlist("pl:followed", "Followed Mix"),
            ])),
            Some("pl:mine") => Ok(paged((1..=3).map(song).collect(), &page)),
            Some("pl:followed") => Ok(paged((4..=5).map(song).collect(), &page)),
            Some("al:1") => Ok(paged((1..=3).map(song).collect(), &page)),
            Some(_) => Err(PluginError::NotFound),
        }
    }

    fn library_likes(kind: ItemKind, page: PageRequest) -> Result<ItemPage> {
        if hostile() {
            let mut items: Vec<Item> = (0..300)
                .map(|n| {
                    let mut item = song(n);
                    item.title = format!("{}{n}", "Long ".repeat(400));
                    item
                })
                .collect();
            items.insert(1, album(1));
            items.insert(2, Item::new(ItemKind::Track, "x".repeat(2000), "Overlong Id"));
            return Ok(ItemPage::with_next(items, "c".repeat(5000)));
        }
        match kind {
            ItemKind::Track => Ok(paged((1..=LIKED_SONGS).map(song).collect(), &page)),
            ItemKind::Album => Ok(paged((1..=2).map(album).collect(), &page)),
            _ => Err(PluginError::Unsupported),
        }
    }

    fn library_playlists(page: PageRequest) -> Result<PlaylistPage> {
        if hostile() {
            let mut entries: Vec<PlaylistEntry> = (0..300)
                .map(|n| PlaylistEntry::owned(playlist(&format!("pl:{n}"), &"P".repeat(5000))))
                .collect();
            entries.insert(0, PlaylistEntry::owned(album(1)));
            return Ok(PlaylistPage::new(entries));
        }
        let all = [
            PlaylistEntry::owned(playlist("pl:mine", "Invented Mix")),
            PlaylistEntry::followed(playlist("pl:followed", "Followed Mix")),
            PlaylistEntry {
                item: playlist("pl:followed-unlisted", "Their Mix"),
                owned: false,
                songs_listed: false,
            },
        ];
        let start: usize = page.cursor.as_deref().and_then(|c| c.parse().ok()).unwrap_or(0);
        let end = (start + page.limit.max(1) as usize).min(all.len());
        let entries = all[start.min(end)..end].to_vec();
        let mut answer = if end < all.len() {
            PlaylistPage::with_next(entries, end.to_string())
        } else {
            PlaylistPage::new(entries)
        };
        answer.total = Some(3);
        Ok(answer)
    }

    fn download(item: Item, quality: DownloadQuality) -> Result<DownloadInfo> {
        if refuse() {
            return Err(PluginError::Unsupported);
        }
        if hostile() {
            let mut info = DownloadInfo::original("https://elsewhere.invalid/stolen.flac", "flac");
            info.file_name = Some("../../escape.flac".into());
            return Ok(info);
        }
        let (container, original) = match quality {
            DownloadQuality::Original => ("flac", true),
            DownloadQuality::High | DownloadQuality::Normal | DownloadQuality::Low => ("opus", false),
        };
        let id = item.id.replace(':', "-");
        let mut info = DownloadInfo::original(format!("{}/files/{id}.{container}", server()), container);
        info.original = original;
        info.headers = vec![Header::new("x-token", "test-token")];
        info.file_name = Some(format!("{id}.{container}"));
        Ok(info)
    }
}

jf_plugin_sdk::export_plugin!(Library);
