//! Tags for downloaded songs (ADR-0025 §2): a converted file gets the song's
//! tags and cover; an original keeps its own and, when asked, gains only a
//! cover and lyrics it lacks.

use std::path::Path;

use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::{Accessor, ItemKey, Tag, TagExt};

use crate::LibraryError;

/// What a downloaded song's tags say.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SongTags {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub year: Option<u32>,
    /// The front cover (JPEG or PNG bytes).
    pub cover: Option<Vec<u8>>,
    /// Lyrics: plain text, or synced lyrics as LRC text.
    pub lyrics: Option<String>,
}

/// Writes `tags` into the audio file at `path` (MP3, FLAC, Opus, Vorbis,
/// M4A and the other formats the library reads). With `only_missing` the
/// file's own tags stay and only a missing cover and missing lyrics are
/// added (Add Missing Cover and Lyrics on an original file).
pub fn write_song_tags(path: &Path, tags: &SongTags, only_missing: bool) -> Result<(), LibraryError> {
    let failed = |message: String| LibraryError::TagsNotWritten {
        path: path.to_path_buf(),
        message,
    };
    let mut file = lofty::read_from_path(path).map_err(|e| failed(e.to_string()))?;
    let tag_type = file.primary_tag_type();
    if file.tag(tag_type).is_none() {
        file.insert_tag(Tag::new(tag_type));
    }
    let tag = file
        .tag_mut(tag_type)
        .ok_or_else(|| failed("the file takes no tags".into()))?;
    if !only_missing {
        tag.set_title(tags.title.clone());
        if let Some(artist) = &tags.artist {
            tag.set_artist(artist.clone());
        }
        if let Some(album) = &tags.album {
            tag.set_album(album.clone());
        }
        if let Some(album_artist) = &tags.album_artist {
            let _ = tag.insert_text(ItemKey::AlbumArtist, album_artist.clone());
        }
        if let Some(track) = tags.track {
            tag.set_track(track);
        }
        if let Some(disc) = tags.disc {
            tag.set_disk(disc);
        }
        if let Some(year) = tags.year {
            let _ = tag.insert_text(ItemKey::RecordingDate, year.to_string());
        }
    }
    if let Some(cover) = &tags.cover
        && (!only_missing || tag.pictures().is_empty())
    {
        let mime = if cover.starts_with(b"\x89PNG") {
            MimeType::Png
        } else {
            MimeType::Jpeg
        };
        tag.remove_picture_type(PictureType::CoverFront);
        tag.push_picture(
            Picture::unchecked(cover.clone())
                .pic_type(PictureType::CoverFront)
                .mime_type(mime)
                .build(),
        );
    }
    if let Some(lyrics) = &tags.lyrics
        && (!only_missing || tag.get_string(ItemKey::Lyrics).is_none())
    {
        let _ = tag.insert_text(ItemKey::Lyrics, lyrics.clone());
    }
    tag.save_to_path(path, WriteOptions::default())
        .map_err(|e| failed(e.to_string()))
}
