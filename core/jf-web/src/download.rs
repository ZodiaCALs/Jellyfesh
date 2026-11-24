//! Download in Jellyfesh Web (ADR-0025 §5): Download on a song saves its file
//! through the browser when its source offers downloads. A browser saves one
//! file in its own downloads folder, so the file is named by the default
//! template's file part ("07 Title.flac"), by the rules every app shares
//! (`jf_plugin_model::filenames`); there is no queue, folder or offline
//! library on the web.

use jf_plugin_model::Item;
use jf_plugin_model::filenames::{self, NameFields};

/// What a song's file is called when the browser saves it: the template's
/// last part for `item` of the source named `source`, with the file's
/// extension (`container`).
pub fn browser_name(item: &Item, source: &str, container: &str) -> String {
    let fields = NameFields {
        artist: item.artist.clone(),
        album_artist: None,
        album: item.album.clone(),
        title: item.title.clone(),
        track: item.track_number,
        disc: item.disc_number,
        year: item.year,
        source: Some(source.to_owned()),
    };
    let parts = filenames::render(filenames::DEFAULT_TEMPLATE, &fields);
    let stem = parts.last().cloned().unwrap_or_default();
    filenames::file_name(&stem, container)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_plugin_model::ItemKind;

    fn song(title: &str, track: Option<u32>) -> Item {
        Item {
            id: "7".into(),
            kind: ItemKind::Track,
            title: title.into(),
            artist: Some("Glass Harbour".into()),
            album: Some("Slow Tide".into()),
            album_id: None,
            artist_id: None,
            duration_ms: None,
            track_number: track,
            disc_number: None,
            year: Some(2025),
            child_count: None,
            isrc: None,
            artwork: None,
            explicit: false,
            playable: true,
            unplayable: None,
        }
    }

    #[test]
    fn names_follow_the_templates_file_part() {
        assert_eq!(
            browser_name(&song("Lanterns", Some(3)), "Jellyfin", "flac"),
            "03 Lanterns.flac"
        );
        assert_eq!(
            browser_name(&song("Lanterns", None), "Jellyfin", ".FLAC"),
            "Lanterns.flac"
        );
        assert_eq!(
            browser_name(&song("What? / Why: <now>", Some(1)), "Jellyfin", "mp3"),
            "01 What_ _ Why_ _now_.mp3"
        );
        assert_eq!(browser_name(&song("CON", None), "Jellyfin", "opus"), "CON_.opus");
        assert_eq!(
            browser_name(&song("  ", Some(2)), "Jellyfin", "m4a"),
            "02 Unknown Title.m4a"
        );
    }
}
