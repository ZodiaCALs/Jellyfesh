//! More about an item (API 1.2, ADR-0022): an album's notes
//! (`getAlbumInfo2`) and facts (`getAlbum`), an artist's biography
//! (`getArtistInfo2`), a song's facts (`getSong`) and a playlist's
//! description (`getPlaylist`), each with the server's own cover. Notes and
//! biographies come from Last.fm through the server, as a little HTML.

use jf_plugin_sdk::prelude::*;

use crate::api::{self, Auth};
use crate::items::Id;
use crate::model::{self, AlbumInfo, Artist, ArtistInfo, Playlist, Song};

/// The size of the large picture.
const PICTURE: &str = "1200";

/// Plain text from notes with a little HTML: tags dropped (a line break for
/// `<br>` and `<p>`), the common entities decoded, spaces collapsed. `None`
/// when nothing is left.
pub fn plain_text(html: &str) -> Option<String> {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        text.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>') else {
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
    let decoded = decode_entities(&text);
    let lines: Vec<String> = decoded
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    let mut out = String::new();
    let mut blank = false;
    for line in lines {
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

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let end = after.find(';').filter(|e| *e <= 8);
        let decoded = end.and_then(|e| {
            let name = &after[..e];
            match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .or_else(|| name.strip_prefix('#').and_then(|n| n.parse().ok()))
                    .and_then(char::from_u32),
            }
        });
        match (decoded, end) {
            (Some(c), Some(e)) => {
                out.push(c);
                rest = &after[e + 1..];
            }
            _ => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn fact(key: FactKey, value: Option<String>) -> Option<Fact> {
    let value = value?.trim().to_string();
    (!value.is_empty()).then_some(Fact { key, value })
}

fn picture(cover: Option<&str>) -> Result<Option<ImageRef>> {
    match cover.filter(|c| !c.is_empty()) {
        Some(cover) => Ok(Some(ImageRef::new(api::media_url(
            "getCoverArt",
            &[("id", cover), ("size", PICTURE)],
        )?))),
        None => Ok(None),
    }
}

/// `getAlbumInfo2` or `getArtistInfo2`, or nothing when the server has no
/// such method or no information (Subsonic errors other than sign-in ones).
fn info<T: serde::de::DeserializeOwned + Default>(
    endpoint: &str,
    id: &str,
    keys: &[&str],
) -> Result<Option<T>> {
    let Some(envelope) = crate::soft(api::request(endpoint, &[("id", id)], Auth::Session))? else {
        return Ok(None);
    };
    for key in keys {
        if let Some(value) = envelope.get(*key).filter(|v| v.is_object()) {
            return Ok(serde_json::from_value(value.clone()).ok());
        }
    }
    Ok(None)
}

/// Names joined with ", ", without repeats.
fn joined(names: impl IntoIterator<Item = String>) -> Option<String> {
    let mut out: Vec<String> = Vec::new();
    for name in names {
        let name = name.trim().to_string();
        if !name.is_empty() && !out.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            out.push(name);
        }
    }
    (!out.is_empty()).then(|| out.join(", "))
}

/// The names of a song's contributors in `role`.
fn role(song: &Song, role: &str) -> Option<String> {
    joined(
        song.contributors
            .iter()
            .filter(|c| c.role.eq_ignore_ascii_case(role))
            .map(|c| c.artist.name.clone()),
    )
}

fn album(id: &str) -> Result<ItemDetails> {
    let album: model::Album = api::call_field("getAlbum", &[("id", id)], "album")?;
    if album.id.is_empty() {
        return Err(PluginError::NotFound);
    }
    let notes: Option<AlbumInfo> = info("getAlbumInfo2", id, &["albumInfo", "albumInfo2"])?;
    let genres = joined(
        album
            .genre
            .iter()
            .cloned()
            .chain(album.genres.iter().map(|g| g.name.clone())),
    );
    let released = album
        .release_date
        .as_ref()
        .and_then(model::ItemDate::text)
        .or_else(|| album.year.filter(|y| *y > 0).map(|y| y.to_string()));
    let facts = [
        fact(FactKey::Version, album.version.clone()),
        fact(FactKey::Genre, genres),
        fact(FactKey::Released, released),
        fact(
            FactKey::Label,
            joined(album.record_labels.iter().map(|l| l.name.clone())),
        ),
    ];
    Ok(ItemDetails {
        description: notes.and_then(|n| n.notes).as_deref().and_then(plain_text),
        image: picture(album.cover_art.as_deref())?,
        facts: facts.into_iter().flatten().collect(),
        comment_count: None,
    })
}

fn artist(id: &str) -> Result<ItemDetails> {
    let artist: Artist = api::call_field("getArtist", &[("id", id)], "artist")?;
    if artist.id.is_empty() {
        return Err(PluginError::NotFound);
    }
    let about: Option<ArtistInfo> = info("getArtistInfo2", id, &["artistInfo2", "artistInfo"])?;
    Ok(ItemDetails {
        description: about.and_then(|a| a.biography).as_deref().and_then(plain_text),
        image: picture(artist.cover_art.as_deref())?,
        facts: Vec::new(),
        comment_count: None,
    })
}

fn track(id: &str) -> Result<ItemDetails> {
    let song: Song = api::call_field("getSong", &[("id", id)], "song")?;
    if song.id.is_empty() {
        return Err(PluginError::NotFound);
    }
    let composer = role(&song, "composer").or_else(|| song.display_composer.clone());
    let facts = [
        fact(FactKey::Genre, song.genre.clone()),
        fact(
            FactKey::Released,
            song.year.filter(|y| *y > 0).map(|y| y.to_string()),
        ),
        fact(FactKey::Composer, composer),
        fact(FactKey::Lyricist, role(&song, "lyricist")),
        fact(FactKey::Arranger, role(&song, "arranger")),
        fact(FactKey::Producer, role(&song, "producer")),
    ];
    Ok(ItemDetails {
        description: None,
        image: picture(song.cover_art.as_deref())?,
        facts: facts.into_iter().flatten().collect(),
        comment_count: None,
    })
}

fn playlist(id: &str) -> Result<ItemDetails> {
    let playlist: Playlist = api::call_field("getPlaylist", &[("id", id)], "playlist")?;
    if playlist.id.is_empty() {
        return Err(PluginError::NotFound);
    }
    Ok(ItemDetails {
        description: playlist.comment.as_deref().and_then(plain_text),
        image: picture(playlist.cover_art.as_deref())?,
        facts: Vec::new(),
        comment_count: None,
    })
}

/// Details of one of the plugin's items.
pub fn details(item: &Item) -> Result<ItemDetails> {
    match Id::parse(&item.id) {
        Some(Id::Album(id)) => album(id),
        Some(Id::Artist(id)) => artist(id),
        Some(Id::Track(id)) => track(id),
        Some(Id::Playlist(id)) => playlist(id),
        Some(Id::Folder(_)) => Err(PluginError::invalid_input("a folder has no details")),
        None => Err(PluginError::NotFound),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_become_plain_text() {
        assert_eq!(
            plain_text(
                "An invented band from the north.<br/>Second line &amp; more. \
                 <a href=\"https://www.last.fm/music/x\">Read more on Last.fm</a>"
            )
            .as_deref(),
            Some("An invented band from the north.\nSecond line & more. Read more on Last.fm")
        );
        assert_eq!(
            plain_text("<p>One</p><p>Two &#233;&#x4E2D;</p>").as_deref(),
            Some("One\n\nTwo é中"),
            "paragraphs stay apart"
        );
        assert_eq!(plain_text(" <a href=\"x\"></a> "), None);
        assert_eq!(
            plain_text("Fish &chips; 3 < 4").as_deref(),
            Some("Fish &chips; 3 < 4")
        );
        assert_eq!(decode_entities("Fish &chips & more"), "Fish &chips & more");
    }

    #[test]
    fn dates_and_names() {
        let date = |y, m, d| model::ItemDate {
            year: y,
            month: m,
            day: d,
        };
        assert_eq!(
            date(Some(2021), Some(3), Some(5)).text().as_deref(),
            Some("2021-03-05")
        );
        assert_eq!(date(Some(2021), Some(3), None).text().as_deref(), Some("2021-03"));
        assert_eq!(date(Some(2021), None, Some(9)).text().as_deref(), Some("2021"));
        assert_eq!(date(Some(0), None, None).text(), None);
        assert_eq!(
            joined([
                "Ambient".to_string(),
                "ambient".into(),
                " ".into(),
                "Drone".into()
            ])
            .as_deref(),
            Some("Ambient, Drone")
        );
    }
}
