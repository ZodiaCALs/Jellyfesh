//! Import Playlist… and Export Playlist… (M3U8) in Jellyfesh Web, by
//! `jf-library`'s rules (`jf_library::playlist`) where a browser allows them:
//!
//! * a playlist file is UTF-8 (with or without a byte-order mark) or UTF-16
//!   with one, at most [`MAX_BYTES`];
//! * blank lines, comments and `#EXT` directives are skipped, and so are URLs
//!   other than `file:`, whose path is percent-decoded (only an empty host or
//!   `localhost` is local); backslashes separate folders, as in playlists
//!   written on Windows;
//! * a page knows no folder's place on the disk, so a line is matched by the
//!   end of its path ([`match_songs`]): the library song whose path (its
//!   folder's name, then where it is in the folder: `Music/Album/01
//!   Song.flac`) ends with the most of the line's folders, and at least its
//!   file name; the same letters in another case count after the exact ones,
//!   and the first song wins a tie. A line no library song matches is left
//!   out (a page cannot read a file it was not given), so the import says how
//!   many it found;
//! * export ([`write`]) writes `#EXTM3U`, then for each library song
//!   `#EXTINF:<seconds>,<artist> - <title>` and its path below its folder's
//!   parent (`Music/Album/01 Song.flac`): kept beside the music folder, the
//!   file reads back in every app. A source's songs have no file and are left
//!   out; a line that would start with `#` starts with `./`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::lyrics::decode_text;

/// The largest playlist file read (as `jf-library`).
pub const MAX_BYTES: usize = 16 << 20;

/// The paths a playlist file names, in order, each as its parts (folders,
/// then the file name). Parts before a `..`, and a drive (`C:`), say nothing
/// about where a song is below its folder, so they are dropped.
pub fn read(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    if bytes.len() > MAX_BYTES {
        return Err("the playlist file is larger than 16 MiB".to_owned());
    }
    Ok(decode_text(bytes).lines().filter_map(line_parts).collect())
}

/// The parts of the file an M3U line names; `None` for blank lines,
/// comments, directives and URLs other than local `file:` ones.
fn line_parts(line: &str) -> Option<Vec<String>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let path = match url_scheme(line) {
        Some(scheme) if scheme.eq_ignore_ascii_case("file") => file_url_path(&line[scheme.len() + 1..])?,
        Some(_) => return None,
        None => line.to_owned(),
    };
    let mut parts: Vec<String> = Vec::new();
    for part in path.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => parts.clear(),
            _ => parts.push(part.to_owned()),
        }
    }
    if parts.first().is_some_and(|first| is_drive(first)) {
        parts.remove(0);
    }
    (!parts.is_empty()).then_some(parts)
}

/// `C:`, a Windows drive.
fn is_drive(part: &str) -> bool {
    let bytes = part.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// The scheme of `scheme://…`; a Windows drive (`C:\`) has no `//` and a
/// one-letter scheme, so it never looks like one.
fn url_scheme(line: &str) -> Option<&str> {
    let end = line.find("://")?;
    let scheme = &line[..end];
    let mut chars = scheme.chars();
    let valid = scheme.len() >= 2
        && chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then_some(scheme)
}

/// `//host/path` of a `file:` URL, percent-decoded; only an empty host or
/// `localhost` is local.
fn file_url_path(rest: &str) -> Option<String> {
    let rest = rest.strip_prefix("//")?;
    let slash = rest.find('/')?;
    let host = &rest[..slash];
    if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
        return None;
    }
    percent_decode(&rest[slash..])
}

/// Decodes `%XX` escapes; `None` for a broken escape or bytes that are not
/// UTF-8.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// For each path [`read`] found, the library song's path (`<folder
/// name>/<path in it>`) that matches it best, or `None`. See the module
/// documentation.
pub fn match_songs(paths: &[Vec<String>], songs: &[String]) -> Vec<Option<String>> {
    let mut by_name: HashMap<String, Vec<&str>> = HashMap::new();
    for song in songs {
        if let Some(name) = song.rsplit('/').next() {
            by_name.entry(name.to_lowercase()).or_default().push(song);
        }
    }
    paths
        .iter()
        .map(|parts| {
            let name = parts.last()?.to_lowercase();
            let mut best: Option<(&str, (usize, usize))> = None;
            for song in by_name.get(&name)? {
                let score = suffix_score(song, parts);
                if best.is_none_or(|(_, top)| score > top) {
                    best = Some((song, score));
                }
            }
            best.map(|(song, _)| song.to_owned())
        })
        .collect()
}

/// How many trailing parts of the song's path match `parts`: exactly, then in
/// any case.
fn suffix_score(song: &str, parts: &[String]) -> (usize, usize) {
    let mut exact = 0;
    let mut any_case = 0;
    let mut still_exact = true;
    for (ours, theirs) in song.rsplit('/').zip(parts.iter().rev()) {
        if still_exact && ours == theirs {
            exact += 1;
        } else {
            still_exact = false;
        }
        if ours.to_lowercase() == theirs.to_lowercase() {
            any_case += 1;
        } else {
            break;
        }
    }
    (exact, any_case)
}

/// A library song as Export Playlist… writes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExportSong {
    /// Its folder's name, then where it is in the folder (`Music/Album/01
    /// Song.flac`).
    pub path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub duration_ms: Option<u64>,
}

/// A playlist as M3U8. See the module documentation.
pub fn write(songs: &[ExportSong]) -> String {
    let mut out = String::from("#EXTM3U\n");
    for song in songs {
        let seconds = song.duration_ms.map_or(-1, |ms| {
            i64::try_from(ms.saturating_add(500) / 1000).unwrap_or(i64::MAX)
        });
        let title = song
            .title
            .clone()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| stem(&song.path));
        let label = match song.artist.as_deref().filter(|artist| !artist.trim().is_empty()) {
            Some(artist) => format!("{artist} - {title}"),
            None => title,
        };
        out.push_str(&format!(
            "#EXTINF:{seconds},{}\n",
            label.replace(['\r', '\n'], " ")
        ));
        if song.path.starts_with('#') {
            out.push_str("./");
        }
        out.push_str(&song.path);
        out.push('\n');
    }
    out
}

/// A file name's stem: "01 Song" of `Music/A/01 Song.flac`.
fn stem(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_owned(),
        _ => name.to_owned(),
    }
}

/// What Import Playlist… found: the paths of the library songs the lines
/// matched, in order, and how many lines named a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Imported {
    pub found: Vec<String>,
    pub lines: usize,
}

/// Reads a playlist file against the library songs' paths.
pub fn import(bytes: &[u8], songs: &[String]) -> Result<Imported, String> {
    let paths = read(bytes)?;
    let found = match_songs(&paths, songs).into_iter().flatten().collect();
    Ok(Imported {
        found,
        lines: paths.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(line: &str) -> Option<Vec<String>> {
        line_parts(line)
    }

    fn owned(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|p| (*p).to_owned()).collect()
    }

    #[test]
    fn lines_name_local_files_as_jf_library_reads_them() {
        assert_eq!(parts("  "), None);
        assert_eq!(parts("#EXTINF:12,A - B"), None);
        assert_eq!(parts("https://example.com/a.mp3"), None);
        assert_eq!(parts("a/b.flac"), Some(owned(&["a", "b.flac"])));
        assert_eq!(parts("../music/b.flac"), Some(owned(&["music", "b.flac"])));
        assert_eq!(parts("sub\\dir\\a.flac"), Some(owned(&["sub", "dir", "a.flac"])));
        assert_eq!(parts("C:\\Music\\x.flac"), Some(owned(&["Music", "x.flac"])));
        assert_eq!(
            parts("/home/me/./Music/x.flac"),
            Some(owned(&["home", "me", "Music", "x.flac"]))
        );
        assert_eq!(url_scheme("C:\\Music\\x.flac"), None);
        assert_eq!(url_scheme("C://x"), None);
        assert_eq!(url_scheme("file:///x"), Some("file"));
        assert_eq!(url_scheme("rtsp+tcp://x"), Some("rtsp+tcp"));
        assert_eq!(
            parts("file:///music/%E6%99%B4%20sky.flac"),
            Some(owned(&["music", "晴 sky.flac"]))
        );
        assert_eq!(
            parts("FILE://localhost/music/a.flac"),
            Some(owned(&["music", "a.flac"]))
        );
        assert_eq!(
            parts("file:///C:/Music/a.flac"),
            Some(owned(&["Music", "a.flac"]))
        );
        assert_eq!(parts("file://server/music/a.flac"), None);
        assert_eq!(parts("file:///music/%zz.flac"), None);
        assert_eq!(parts("file:///music/%ff.flac"), None, "not UTF-8");
    }

    #[test]
    fn files_are_read_in_any_encoding_up_to_the_limit() {
        let text = "#EXTM3U\r\n#EXTINF:3,A - B\r\nTide/01 Harbour.wav\r\n";
        assert_eq!(
            read(text.as_bytes()).unwrap(),
            [owned(&["Tide", "01 Harbour.wav"])]
        );
        let mut utf16 = vec![0xFF, 0xFE];
        utf16.extend("Solo.wav\n".encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(read(&utf16).unwrap(), [owned(&["Solo.wav"])]);
        assert!(read(&vec![b'a'; MAX_BYTES + 1]).is_err());
    }

    #[test]
    fn lines_match_the_song_whose_path_ends_like_them() {
        let songs = owned(&[
            "Music/Tide/01 Harbour.wav",
            "Music/Loose/Solo.wav",
            "Other/Loose/Solo.wav",
            "Music/Tide/02 Lantern.wav",
        ]);
        let found = |line: &str| match_songs(&[line_parts(line).unwrap()], &songs).remove(0);
        assert_eq!(
            found("C:\\Users\\me\\Music\\Tide\\01 Harbour.wav").as_deref(),
            Some("Music/Tide/01 Harbour.wav")
        );
        assert_eq!(
            found("../Other/Loose/Solo.wav").as_deref(),
            Some("Other/Loose/Solo.wav"),
            "more folders match"
        );
        assert_eq!(
            found("Solo.wav").as_deref(),
            Some("Music/Loose/Solo.wav"),
            "the first of equals"
        );
        assert_eq!(
            found("tide/02 LANTERN.WAV").as_deref(),
            Some("Music/Tide/02 Lantern.wav"),
            "another case"
        );
        assert_eq!(found("Music/Tide/03 Missing.wav"), None);
        let imported = import(
            b"#EXTM3U\nLoose/Solo.wav\nnowhere.mp3\nhttp://x/y.mp3\nTide/01 Harbour.wav\n",
            &songs,
        )
        .unwrap();
        assert_eq!(
            imported.found,
            ["Music/Loose/Solo.wav", "Music/Tide/01 Harbour.wav"]
        );
        assert_eq!(imported.lines, 3);
    }

    #[test]
    fn export_writes_extinf_and_paths_below_the_folders_parent() {
        let songs = [
            ExportSong {
                path: "Music/Tide/01 Harbour.wav".into(),
                title: Some("Harbour".into()),
                artist: Some("Aurora".into()),
                duration_ms: Some(4_499),
            },
            ExportSong {
                path: "#1/Loose/Solo.wav".into(),
                title: None,
                artist: None,
                duration_ms: None,
            },
            ExportSong {
                path: "Music/Odd.wav".into(),
                title: Some("Two\nLines".into()),
                artist: Some(" ".into()),
                duration_ms: Some(500),
            },
        ];
        assert_eq!(
            write(&songs),
            "#EXTM3U\n#EXTINF:4,Aurora - Harbour\nMusic/Tide/01 Harbour.wav\n#EXTINF:-1,Solo\n./#1/Loose/Solo.wav\n#EXTINF:1,Two Lines\nMusic/Odd.wav\n"
        );
        // What it writes reads back to the same songs.
        let paths: Vec<String> = songs.iter().map(|s| s.path.clone()).collect();
        assert_eq!(import(write(&songs).as_bytes(), &paths).unwrap().found, paths);
    }
}
