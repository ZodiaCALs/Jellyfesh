//! More about an item (API 1.2, ADR-0022): its `Overview` (album notes, an
//! artist's biography, a playlist's description), genres, release date,
//! labels (`Studios`) and the people on a song (`People`), with its large
//! picture. One `/Items` request.

use jf_plugin_sdk::prelude::*;

use crate::api;
use crate::items::{self, Id};
use crate::model::{BaseItem, ItemList};

/// The large picture's size.
const PICTURE: u32 = 1200;

/// Plain text from an overview, which metadata providers sometimes send
/// with a little HTML: tags dropped (a line break for `<br>` and `<p>`),
/// the common entities decoded, spaces collapsed. `None` when empty.
pub fn plain_text(html: &str) -> Option<String> {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        text.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>') else {
            // A lone "<": text, not a tag.
            rest = &rest[open..];
            break;
        };
        let tag = rest[open + 1..open + close]
            .trim_start_matches('/')
            .to_ascii_lowercase();
        if tag.starts_with("br") || tag == "p" || tag.starts_with("p ") {
            text.push('\n');
        }
        rest = &rest[open + close + 1..];
    }
    text.push_str(rest);
    let text = text
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ");
    let mut out = String::new();
    let mut blank = false;
    for line in text
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
    {
        if line.is_empty() {
            blank = !out.is_empty();
            continue;
        }
        if !out.is_empty() {
            out.push_str(if blank { "\n\n" } else { "\n" });
        }
        blank = false;
        out.push_str(&line);
    }
    (!out.is_empty()).then_some(out)
}

fn joined<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut out: Vec<&str> = Vec::new();
    for name in names.into_iter().map(str::trim) {
        if !name.is_empty() && !out.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            out.push(name);
        }
    }
    (!out.is_empty()).then(|| out.join(", "))
}

fn fact(key: FactKey, value: Option<String>) -> Option<Fact> {
    value.map(|value| Fact { key, value })
}

/// The people of `kind` on an item.
fn people(item: &BaseItem, kind: &str) -> Option<String> {
    joined(
        item.people
            .iter()
            .filter(|p| p.kind.as_deref().is_some_and(|k| k.eq_ignore_ascii_case(kind)))
            .filter_map(|p| p.name.as_deref()),
    )
}

/// The release date (`2021-03-05`), else the year.
fn released(item: &BaseItem) -> Option<String> {
    item.premiere_date
        .as_deref()
        .and_then(|d| d.get(..10))
        .filter(|d| d.len() == 10 && d.as_bytes()[4] == b'-' && d.as_bytes()[7] == b'-')
        .map(str::to_owned)
        .or_else(|| item.production_year.filter(|y| *y > 0).map(|y| y.to_string()))
}

/// What the plugin shows about `item`, a Jellyfin item.
pub fn of(item: &BaseItem) -> Result<ItemDetails> {
    let facts = [
        fact(FactKey::Genre, joined(item.genres.iter().map(String::as_str))),
        fact(FactKey::Released, released(item)),
        fact(
            FactKey::Label,
            joined(item.studios.iter().filter_map(|s| s.name.as_deref())),
        ),
        fact(FactKey::Composer, people(item, "Composer")),
        fact(FactKey::Lyricist, people(item, "Lyricist")),
        fact(FactKey::Arranger, people(item, "Arranger")),
        fact(FactKey::Producer, people(item, "Producer")),
    ];
    let image = match items::image(item) {
        Some((image_item, tag)) => Some(ImageRef::new(items::image_url(&image_item, Some(&tag), PICTURE)?)),
        None => None,
    };
    Ok(ItemDetails {
        description: item.overview.as_deref().and_then(plain_text),
        image,
        facts: facts.into_iter().flatten().collect(),
        comment_count: None,
    })
}

/// Details of one of the plugin's items.
pub fn details(item: &Item) -> Result<ItemDetails> {
    let Some(id) = Id::parse(&item.id).and_then(Id::server_id) else {
        return Err(if Id::parse(&item.id).is_some() {
            PluginError::invalid_input("a folder has no details")
        } else {
            PluginError::NotFound
        });
    };
    let found: ItemList = api::get(
        "/Items",
        &[
            ("ids", id),
            ("fields", "Overview,Genres,People,Studios"),
            ("enableImageTypes", "Primary"),
            ("imageTypeLimit", "1"),
        ],
    )?;
    let found = found
        .items
        .into_iter()
        .find(|i| i.id.eq_ignore_ascii_case(id))
        .ok_or(PluginError::NotFound)?;
    of(&found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NameId, Person};

    #[test]
    fn overviews_become_plain_text() {
        assert_eq!(
            plain_text("Invented notes.<br>More &amp; more.").as_deref(),
            Some("Invented notes.\nMore & more.")
        );
        assert_eq!(plain_text("  "), None);
        assert_eq!(
            plain_text("Plain text stays").as_deref(),
            Some("Plain text stays")
        );
        assert_eq!(plain_text("3 < 4").as_deref(), Some("3 < 4"));
    }

    #[test]
    fn facts_from_genres_dates_studios_and_people() {
        let item = BaseItem {
            id: "a1".into(),
            genres: vec!["Ambient".into(), "ambient".into(), "Folk".into()],
            premiere_date: Some("2021-03-05T00:00:00.0000000Z".into()),
            production_year: Some(2021),
            studios: vec![NameId {
                name: Some("Invented Label".into()),
                id: "s1".into(),
            }],
            people: vec![
                Person {
                    name: Some("Ada Invented".into()),
                    kind: Some("Composer".into()),
                },
                Person {
                    name: Some("Bo Invented".into()),
                    kind: Some("Lyricist".into()),
                },
                Person {
                    name: Some("Cy Invented".into()),
                    kind: Some("Actor".into()),
                },
            ],
            ..BaseItem::default()
        };
        let facts: Vec<(FactKey, String)> = [
            fact(FactKey::Genre, joined(item.genres.iter().map(String::as_str))),
            fact(FactKey::Released, released(&item)),
            fact(FactKey::Composer, people(&item, "Composer")),
            fact(FactKey::Lyricist, people(&item, "Lyricist")),
        ]
        .into_iter()
        .flatten()
        .map(|f| (f.key, f.value))
        .collect();
        assert_eq!(
            facts,
            [
                (FactKey::Genre, "Ambient, Folk".to_string()),
                (FactKey::Released, "2021-03-05".into()),
                (FactKey::Composer, "Ada Invented".into()),
                (FactKey::Lyricist, "Bo Invented".into())
            ]
        );
        let year_only = BaseItem {
            production_year: Some(1999),
            premiere_date: Some("garbage".into()),
            ..BaseItem::default()
        };
        assert_eq!(released(&year_only).as_deref(), Some("1999"));
    }
}
