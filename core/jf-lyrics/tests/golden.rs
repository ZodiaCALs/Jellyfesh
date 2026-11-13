//! Golden tests: every input file under `tests/golden/<format>/` is parsed and
//! compared with the `.json` file of the same name next to it (the pretty JSON
//! of the `LyricDoc`, or of the error for inputs that must fail).
//!
//! A mismatch fails with a diff. To accept new output, run
//! `UPDATE_GOLDEN=1 cargo test -p jf-lyrics --test golden` and review the
//! changed JSON files like any other change.

use std::fs;
use std::path::{Path, PathBuf};

use jf_lyrics::{Format, LyricDoc, parse};

const FORMATS: [(&str, Format); 4] = [
    ("lrc", Format::Lrc),
    ("ttml", Format::Ttml),
    ("vtt", Format::WebVtt),
    ("plain", Format::Plain),
];

#[test]
fn golden_files_match() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let update = std::env::var("UPDATE_GOLDEN").is_ok_and(|v| v == "1");
    let mut failures = Vec::new();
    let mut cases = 0;
    for (dir, format) in FORMATS {
        let mut files: Vec<PathBuf> = fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("reading {dir}: {e}"))
            .map(|entry| entry.unwrap().path())
            .collect();
        files.sort();
        for path in &files {
            let is_json = path.extension().is_some_and(|e| e == "json");
            if is_json {
                if !files
                    .iter()
                    .any(|p| p != path && p.with_extension("json") == *path)
                {
                    failures.push(format!("{}: no input file for this golden file", name(path)));
                }
                continue;
            }
            cases += 1;
            let input = String::from_utf8(fs::read(path).unwrap()).expect("fixtures are UTF-8");
            let actual = render(&input, format, path);
            let json_path = path.with_extension("json");
            let expected = fs::read_to_string(&json_path)
                .ok()
                .map(|s| s.replace("\r\n", "\n"));
            if expected.as_deref() == Some(actual.as_str()) {
                continue;
            }
            if update {
                fs::write(&json_path, &actual).unwrap();
                eprintln!("updated {}", name(&json_path));
            } else {
                failures.push(format!(
                    "{}:\n{}",
                    name(&json_path),
                    diff(expected.as_deref().unwrap_or(""), &actual)
                ));
            }
        }
    }
    assert!(cases >= 20, "expected the golden fixtures, found {cases}");
    assert!(
        failures.is_empty(),
        "{} golden file(s) differ; run with UPDATE_GOLDEN=1 to accept the new output:\n\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn name(path: &Path) -> String {
    let parent = path.parent().and_then(Path::file_name).unwrap_or_default();
    format!(
        "{}/{}",
        parent.to_string_lossy(),
        path.file_name().unwrap_or_default().to_string_lossy()
    )
}

/// Pretty JSON of the parse result, after checking the document's invariants.
fn render(input: &str, format: Format, path: &Path) -> String {
    let json = match parse(input, Some(format)) {
        Ok(doc) => {
            check_invariants(&doc, path);
            let json = serde_json::to_string_pretty(&doc).unwrap();
            let back: LyricDoc = serde_json::from_str(&json).unwrap();
            assert_eq!(back, doc, "{}: JSON round trip", path.display());
            json
        }
        Err(err) => serde_json::to_string_pretty(&serde_json::json!({
            "error": err,
            "message": err.to_string(),
        }))
        .unwrap(),
    };
    json + "\n"
}

fn check_invariants(doc: &LyricDoc, path: &Path) {
    let p = path.display();
    for (i, line) in doc.lines.iter().enumerate() {
        assert_eq!(line.id as usize, i, "{p}: ids are positions");
        assert_eq!(line.start_ms.is_some(), doc.is_synced(), "{p}: line {i} start");
        if let (Some(s), Some(e)) = (line.start_ms, line.end_ms) {
            assert!(e >= s, "{p}: line {i} ends before it starts");
        }
        if i > 0 {
            assert!(
                line.start_ms >= doc.lines[i - 1].start_ms,
                "{p}: lines are sorted"
            );
        }
        for words in [&line.words, &line.background] {
            for (j, w) in words.iter().enumerate() {
                assert!(
                    w.end_ms >= w.start_ms,
                    "{p}: line {i} word {j} ends before it starts"
                );
                assert!(
                    j == 0 || w.start_ms >= words[j - 1].start_ms,
                    "{p}: line {i} word starts decrease"
                );
                assert!(!w.text.is_empty(), "{p}: line {i} has an empty word");
            }
        }
        if let Some(agent) = &line.agent {
            assert!(
                doc.agents.iter().any(|a| &a.id == agent),
                "{p}: unknown agent {agent}"
            );
        }
    }
    for track in &doc.tracks {
        assert!(
            track.lines.keys().all(|&id| (id as usize) < doc.lines.len()),
            "{p}: track keys"
        );
    }
    let tl = doc.timeline();
    assert!(
        tl.segment_starts.windows(2).all(|w| w[0] < w[1]),
        "{p}: segment breakpoints"
    );
}

/// The first differing region, `-` for expected and `+` for actual lines.
fn diff(expected: &str, actual: &str) -> String {
    let e: Vec<&str> = expected.lines().collect();
    let a: Vec<&str> = actual.lines().collect();
    let first = e
        .iter()
        .zip(&a)
        .position(|(x, y)| x != y)
        .unwrap_or(e.len().min(a.len()));
    let mut out = String::new();
    for i in first.saturating_sub(3)..(first + 12).min(e.len().max(a.len())) {
        match (e.get(i), a.get(i)) {
            (Some(x), Some(y)) if x == y => out.push_str(&format!("    {x}\n")),
            (x, y) => {
                if let Some(x) = x {
                    out.push_str(&format!("  - {x}\n"));
                }
                if let Some(y) = y {
                    out.push_str(&format!("  + {y}\n"));
                }
            }
        }
    }
    out
}
