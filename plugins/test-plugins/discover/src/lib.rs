//! A test plugin for the API 1.2 exports `discover`, `details` and
//! `comments`, with invented items and words. Its pictures live on the test
//! server named by `server_url` and need the header `x-art-key: test-key`,
//! as some services' covers do. With `hostile` on, every answer breaks the
//! host's limits (too many shelves, items, facts and comments, overlong
//! texts) so the tests can see them cut.

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;

struct Discover;

thread_local! {
    static SERVER: RefCell<String> = const { RefCell::new(String::new()) };
    static HOSTILE: RefCell<bool> = const { RefCell::new(false) };
}

fn server() -> String {
    SERVER.with(|s| s.borrow().trim_end_matches('/').to_string())
}

fn hostile() -> bool {
    HOSTILE.with(|h| *h.borrow())
}

/// A picture on the test server that needs the header.
fn picture(name: &str) -> ImageRef {
    ImageRef {
        url: format!("{}/art/{name}.png", server()),
        headers: vec![Header::new("x-art-key", "test-key")],
        width: Some(300),
        height: Some(300),
    }
}

fn album(n: usize) -> Item {
    let mut item = Item::new(ItemKind::Album, format!("al:{n}"), format!("Invented Album {n}"))
        .artist("Invented Artist");
    item.artist_id = Some("ar:1".into());
    item.artwork = Some(picture(&format!("al-{n}")));
    item
}

fn song(n: usize) -> Item {
    let mut item = Item::new(ItemKind::Track, format!("tr:{n}"), format!("Invented Song {n}"))
        .artist("Invented Artist")
        .album("Invented Album 1")
        .duration_ms(180_000);
    item.album_id = Some("al:1".into());
    item.artist_id = Some("ar:1".into());
    item.artwork = Some(picture(&format!("tr-{n}")));
    item
}

fn shelf(id: &str, title: &str, style: ShelfStyle, items: Vec<Item>, more: Option<&str>) -> Shelf {
    Shelf {
        id: id.into(),
        title: title.into(),
        style,
        items,
        more: more.map(Into::into),
    }
}

fn comment(id: &str, text: &str) -> Comment {
    Comment {
        id: id.into(),
        author: "Invented Listener".into(),
        avatar: Some(picture("face")),
        text: text.into(),
        likes: Some(7),
        time_ms: Some(1_700_000_000_000),
        replies: Some(1),
    }
}

impl Plugin for Discover {
    fn capabilities() -> Vec<Capability> {
        vec![Capability::Browse]
    }

    fn settings_changed(settings: &serde_json::Value) -> Result<()> {
        let url = settings["server_url"].as_str().unwrap_or_default().to_string();
        SERVER.with(|s| *s.borrow_mut() = url);
        HOSTILE.with(|h| *h.borrow_mut() = settings["hostile"].as_bool().unwrap_or(false));
        Ok(())
    }

    fn browse(node: Option<Item>, _page: PageRequest) -> Result<ItemPage> {
        match node.as_ref().map(|n| n.id.as_str()) {
            None => Ok(ItemPage::new(vec![Item::new(
                ItemKind::Folder,
                "f:all",
                "All Songs",
            )])),
            Some("chart:top" | "f:all") => Ok(ItemPage::new((1..=8).map(song).collect())),
            Some("al:1") => Ok(ItemPage::new((1..=3).map(song).collect())),
            Some("ar:1") => Ok(ItemPage::new((1..=2).map(album).collect())),
            Some(_) => Err(PluginError::NotFound),
        }
    }

    fn discover() -> Result<Vec<Shelf>> {
        if hostile() {
            let mut shelves = vec![shelf("nameless", " ", ShelfStyle::Row, vec![song(1)], None)];
            shelves.extend((0..20).map(|i| {
                shelf(
                    &format!("s{i}"),
                    "Too Many",
                    ShelfStyle::Grid,
                    (0..80).map(song).collect(),
                    None,
                )
            }));
            return Ok(shelves);
        }
        Ok(vec![
            shelf(
                "new",
                "New Releases",
                ShelfStyle::Row,
                (1..=3).map(album).collect(),
                None,
            ),
            shelf(
                "top",
                "Top Songs",
                ShelfStyle::Chart,
                (1..=5).map(song).collect(),
                Some("chart:top"),
            ),
        ])
    }

    fn details(item: Item) -> Result<ItemDetails> {
        if hostile() {
            return Ok(ItemDetails {
                description: Some("x".repeat(50_000)),
                image: Some(ImageRef::new("https://elsewhere.invalid/photo.png")),
                facts: (0..40)
                    .map(|i| Fact {
                        key: FactKey::Other(format!("Fact {i}")),
                        value: "Invented".into(),
                    })
                    .collect(),
                comment_count: None,
            });
        }
        let description = match item.kind {
            ItemKind::Artist => format!("An invented biography of {}.", item.title),
            _ => format!("Invented notes about {}.", item.title),
        };
        Ok(ItemDetails {
            description: Some(description),
            image: Some(picture(&format!("{}-large", item.id.replace(':', "-")))),
            facts: vec![
                Fact {
                    key: FactKey::Released,
                    value: "2026-01-02".into(),
                },
                Fact {
                    key: FactKey::Genre,
                    value: "Invented Pop".into(),
                },
                Fact {
                    key: FactKey::Other("Mood".into()),
                    value: "Calm".into(),
                },
            ],
            comment_count: Some(3),
        })
    }

    fn comments(_item: Item, page: PageRequest) -> Result<CommentPage> {
        if hostile() {
            return Ok(CommentPage {
                hot: (0..60).map(|i| comment(&format!("h{i}"), "Hot")).collect(),
                comments: (0..80)
                    .map(|i| comment(&format!("c{i}"), &"y".repeat(5000)))
                    .collect(),
                total: None,
                next: None,
            });
        }
        match page.cursor.as_deref() {
            None => Ok(CommentPage {
                hot: vec![comment("h1", "An invented hot comment.")],
                comments: vec![comment("c1", "First invented comment."), comment("c2", "Second.")],
                total: Some(3),
                next: Some("2".into()),
            }),
            Some("2") => Ok(CommentPage {
                hot: Vec::new(),
                comments: vec![comment("c3", "The last invented comment.")],
                total: Some(3),
                next: None,
            }),
            Some(_) => Err(PluginError::NotFound),
        }
    }
}

jf_plugin_sdk::export_plugin!(Discover);
