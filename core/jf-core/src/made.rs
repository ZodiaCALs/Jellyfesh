//! Lyrics AI Lyrics made (ADR-0019 §4), kept in `<data>/made-lyrics/`.
//!
//! Each song has two files named by the blake3 of what identifies it (its
//! file, or its source and the source's id): `<key>.lrc` holds the lyrics as
//! the plugin made them (LRC with word times, or another format the meta
//! names) and `<key>.json` who made them and when. They are shown when the
//! song has no lyrics of its own, or only unsynced ones, and kept until the
//! person deletes them (Delete Made Lyrics) or the data folder goes.

use std::path::{Path, PathBuf};

use jf_lyrics::LyricDoc;
use jf_session::TrackRef;
use serde::{Deserialize, Serialize};

/// Most bytes of a made lyrics file read back.
const MAX_LYRICS: u64 = 2 * 1024 * 1024;

/// What a made lyrics file says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MadeMeta {
    /// The plugin that made them.
    pub plugin: String,
    /// The engine, in the plugin's words ("Whisper Base on this device").
    pub engine: String,
    /// The song, for people looking in the folder.
    pub title: String,
    pub artist: Option<String>,
    /// The lyrics' format: `lrc`, `elrc`, `ttml`, `vtt` or `plain`.
    pub format: String,
    /// When they were made, in ms since the Unix epoch.
    pub made_at_ms: i64,
}

/// The made-lyrics folder.
#[derive(Debug, Clone)]
pub(crate) struct MadeLyrics {
    dir: PathBuf,
}

/// What identifies a song across library scans and queues: its file, or its
/// source and the source's id. `None` for songs that cannot have made lyrics
/// (a library id the caller could not resolve to a file).
pub(crate) fn identity(track: &TrackRef, library_path: Option<&Path>) -> Option<String> {
    match track {
        TrackRef::Library(_) => library_path.map(file_identity),
        TrackRef::File(path) => Some(file_identity(path)),
        TrackRef::Source(track) => Some(source_identity(&track.plugin, &track.id)),
    }
}

/// What identifies the song in a file.
pub(crate) fn file_identity(path: &Path) -> String {
    format!("file\n{}", path.display())
}

/// What identifies a source's song.
pub(crate) fn source_identity(plugin: &str, id: &str) -> String {
    format!("source\n{plugin}\n{id}")
}

/// The format name stored for a jf-lyrics format.
pub(crate) fn format_name(format: jf_lyrics::Format) -> &'static str {
    match format {
        jf_lyrics::Format::Lrc => "lrc",
        jf_lyrics::Format::EnhancedLrc => "elrc",
        jf_lyrics::Format::Ttml => "ttml",
        jf_lyrics::Format::WebVtt => "vtt",
        _ => "plain",
    }
}

fn format_of(name: &str) -> jf_lyrics::Format {
    match name {
        "lrc" => jf_lyrics::Format::Lrc,
        "elrc" => jf_lyrics::Format::EnhancedLrc,
        "ttml" => jf_lyrics::Format::Ttml,
        "vtt" => jf_lyrics::Format::WebVtt,
        _ => jf_lyrics::Format::Plain,
    }
}

impl MadeLyrics {
    pub(crate) fn new(data_dir: &Path) -> MadeLyrics {
        MadeLyrics {
            dir: data_dir.join("made-lyrics"),
        }
    }

    fn key(identity: &str) -> String {
        blake3::hash(identity.as_bytes()).to_hex()[..32].to_owned()
    }

    fn paths(&self, identity: &str) -> (PathBuf, PathBuf) {
        let key = Self::key(identity);
        (
            self.dir.join(format!("{key}.lrc")),
            self.dir.join(format!("{key}.json")),
        )
    }

    /// The made lyrics for a song, read and parsed, with who made them.
    pub(crate) fn get(&self, identity: &str) -> Option<(LyricDoc, MadeMeta)> {
        let (lyrics, meta) = self.paths(identity);
        let meta: MadeMeta = serde_json::from_slice(&std::fs::read(meta).ok()?).ok()?;
        if std::fs::metadata(&lyrics).ok()?.len() > MAX_LYRICS {
            return None;
        }
        let text = std::fs::read_to_string(&lyrics).ok()?;
        match jf_lyrics::parse(&text, Some(format_of(&meta.format))) {
            Ok(mut doc) => {
                doc.source.origin = Some(meta.engine.clone());
                Some((doc, meta))
            }
            Err(error) => {
                tracing::warn!(%error, "made lyrics could not be read");
                None
            }
        }
    }

    /// Whether a song has made lyrics.
    pub(crate) fn has(&self, identity: &str) -> bool {
        let (lyrics, meta) = self.paths(identity);
        lyrics.is_file() && meta.is_file()
    }

    /// Keeps lyrics made for a song, replacing any made before.
    pub(crate) fn put(&self, identity: &str, text: &str, meta: &MadeMeta) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let (lyrics, meta_path) = self.paths(identity);
        crate::sources::write_atomic(&lyrics, text.as_bytes())?;
        let json = serde_json::to_vec_pretty(meta).map_err(std::io::Error::other)?;
        crate::sources::write_atomic(&meta_path, &json)
    }

    /// Deletes a song's made lyrics; says whether there were any.
    pub(crate) fn delete(&self, identity: &str) -> std::io::Result<bool> {
        let (lyrics, meta) = self.paths(identity);
        let mut found = false;
        for path in [lyrics, meta] {
            match std::fs::remove_file(path) {
                Ok(()) => found = true,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(format: &str) -> MadeMeta {
        MadeMeta {
            plugin: "org.example.maker".into(),
            engine: "Test Engine".into(),
            title: "Invented Song".into(),
            artist: None,
            format: format.into(),
            made_at_ms: 1,
        }
    }

    #[test]
    fn keeps_reads_and_deletes_made_lyrics() {
        let dir = tempfile::tempdir().unwrap();
        let made = MadeLyrics::new(dir.path());
        let song = identity(&TrackRef::File("/music/a.flac".into()), None).unwrap();
        let other = identity(&TrackRef::Library(3), Some(Path::new("/music/b.flac"))).unwrap();
        assert_eq!(identity(&TrackRef::Library(3), None), None);
        assert!(made.get(&song).is_none());
        // Invented lyrics with word times.
        let text = "[00:01.00]<00:01.00>la <00:01.50>la\n[00:03.00]<00:03.00>da\n";
        made.put(&song, text, &meta("elrc")).unwrap();
        assert!(made.has(&song));
        assert!(!made.has(&other));
        let (doc, read) = made.get(&song).unwrap();
        assert_eq!(read, meta("elrc"));
        assert_eq!(doc.lines.len(), 2);
        assert_eq!(doc.source.origin.as_deref(), Some("Test Engine"));
        assert!(doc.timeline().word_starts.len() >= 3);
        // The same file as a library song shares them.
        let same = identity(&TrackRef::Library(9), Some(Path::new("/music/a.flac"))).unwrap();
        assert!(made.has(&same));
        assert!(made.delete(&song).unwrap());
        assert!(!made.delete(&song).unwrap());
        assert!(made.get(&song).is_none());
    }
}
