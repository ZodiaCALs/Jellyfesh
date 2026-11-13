//! Turning files and folders into queue entries.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use jf_session::TrackRef;

use crate::actor::is_audio_path;

/// Folders nested deeper than this are not walked (guards symlink loops).
const MAX_DEPTH: usize = 16;

/// Expands files and folders into queue entries: audio files as given,
/// folders walked recursively in natural order ("2" before "10"), hidden
/// files and folders skipped.
pub fn expand_paths(paths: &[PathBuf]) -> Vec<TrackRef> {
    let mut out = Vec::new();
    for path in paths {
        if path.is_dir() {
            let mut found = Vec::new();
            walk(path, 0, &mut found);
            found.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
            out.extend(found.into_iter().map(TrackRef::File));
        } else if path.is_file() {
            out.push(TrackRef::File(
                std::path::absolute(path).unwrap_or_else(|_| path.clone()),
            ));
        }
    }
    out
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if path.is_dir() {
            walk(&path, depth + 1, out);
        } else if is_audio_path(&path) {
            out.push(std::path::absolute(&path).unwrap_or(path));
        }
    }
}

/// Compares strings with runs of digits compared by value, case-insensitively
/// otherwise: "Track 2" < "Track 10". Case and zero padding only break ties
/// between strings that are otherwise equal.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    let mut tie = Ordering::Equal;
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return tie,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (da, db) = (digits(&mut a), digits(&mut b));
                let (ta, tb) = (da.trim_start_matches('0'), db.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
                tie = tie.then(da.len().cmp(&db.len()));
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                tie = tie.then(x.cmp(&y));
                a.next();
                b.next();
            }
        }
    }
}

fn digits(it: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut out = String::new();
    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
        out.push(c);
        it.next();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut v = vec![
            "Track 10.flac",
            "track 2.flac",
            "Track 1.flac",
            "Track 02.flac",
            "B",
            "a",
        ];
        v.sort_by(|a, b| natural_cmp(a, b));
        // "Track 02" and "track 2" differ only in case and padding; the first
        // difference (case: "T" before "t") decides.
        let want = [
            "a",
            "B",
            "Track 1.flac",
            "Track 02.flac",
            "track 2.flac",
            "Track 10.flac",
        ];
        assert_eq!(v, want);
    }

    #[test]
    fn expands_folders_in_order_and_skips_hidden_and_non_audio() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::fs::create_dir_all(d.join("CD2")).unwrap();
        std::fs::create_dir_all(d.join(".hidden")).unwrap();
        for f in [
            "10 ten.flac",
            "2 two.mp3",
            "notes.txt",
            "CD2/1 one.ogg",
            ".hidden/x.flac",
            ".dot.flac",
        ] {
            std::fs::write(d.join(f), b"").unwrap();
        }
        let got: Vec<String> = expand_paths(&[d.to_path_buf()])
            .into_iter()
            .map(|t| match t {
                TrackRef::File(p) => p.strip_prefix(d).unwrap().to_string_lossy().replace('\\', "/"),
                TrackRef::Library(_) | TrackRef::Source(_) => unreachable!(),
            })
            .collect();
        assert_eq!(got, vec!["2 two.mp3", "10 ten.flac", "CD2/1 one.ogg"]);
    }
}
