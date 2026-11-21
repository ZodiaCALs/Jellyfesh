//! Golden tests for the YRC conversion (ADR-0022 §6, Tests).
//!
//! Every `tests/golden/yrc/<case>.yrc` is invented test data in NetEase's
//! word-timed format (a first `#` line says what the case is about; the
//! converter skips such lines), with an optional translation in
//! `<case>.trans.lrc`. Its enhanced LRC is compared with `<case>.elrc`, and
//! must read back through the core's lyrics parser with every sung line
//! timed word by word (and the translation as a translation track).
//!
//! A mismatch fails with both texts. To accept new output, run
//! `UPDATE_GOLDEN=1 cargo test -p jf-community-netease yrc_golden` and
//! review the changed `.elrc` files like any other change.

use std::fs;
use std::path::{Path, PathBuf};

use crate::yrc;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/yrc")
}

fn cases() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "yrc"))
        .collect();
    files.sort();
    files
}

/// The core reads `text` as enhanced LRC: as many sung lines as the YRC
/// has, each timed word by word in order, and a translation line for each
/// translated one.
fn check_reads_back(name: &str, text: &str, sung: usize, translated: usize) {
    let doc = jf_lyrics::parse(text, Some(jf_lyrics::Format::EnhancedLrc))
        .unwrap_or_else(|e| panic!("{name}: the core cannot read it: {e}"));
    assert_eq!(doc.timing, jf_lyrics::Timing::Word, "{name}: word timing");
    let timed: Vec<&jf_lyrics::Line> = doc.lines.iter().filter(|l| !l.words.is_empty()).collect();
    assert_eq!(timed.len(), sung, "{name}: every sung line, word by word");
    for line in timed {
        let mut last = 0;
        for word in &line.words {
            assert!(word.start_ms >= last, "{name}: {:?} goes back in time", word.text);
            assert!(
                word.end_ms >= word.start_ms,
                "{name}: {:?} ends before it starts",
                word.text
            );
            last = word.start_ms;
        }
    }
    let translations: usize = doc.tracks.iter().map(|t| t.lines.len()).sum();
    assert_eq!(translations, translated, "{name}: translation lines");
}

#[test]
fn golden_files_match() {
    let update = std::env::var("UPDATE_GOLDEN").is_ok_and(|v| v == "1");
    let mut failures = Vec::new();
    let files = cases();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let source = fs::read_to_string(path).unwrap();
        assert!(
            source.starts_with("# Invented test data"),
            "{name}: say that the case is invented"
        );
        let translation = fs::read_to_string(path.with_extension("trans.lrc")).ok();
        let Some(made) = yrc::to_elrc(&source, translation.as_deref()) else {
            failures.push(format!("{name}: nothing word-timed"));
            continue;
        };
        let sung = yrc::parse(&source).iter().filter(|l| !l.words.is_empty()).count();
        let translated = made.lines().count() - yrc::parse(&source).len();
        check_reads_back(&name, &made, sung, translated);
        let golden = path.with_extension("elrc");
        let expected = fs::read_to_string(&golden).ok().map(|s| s.replace("\r\n", "\n"));
        if expected.as_deref() == Some(made.as_str()) {
            continue;
        }
        if update {
            fs::write(&golden, &made).unwrap();
            eprintln!("updated {}", golden.display());
        } else {
            failures.push(format!(
                "{name}:\n--- expected\n{}\n--- made\n{made}",
                expected.unwrap_or_default()
            ));
        }
    }
    assert!(
        files.len() >= 8,
        "expected the golden cases, found {}",
        files.len()
    );
    assert!(
        failures.is_empty(),
        "{} golden case(s) differ; run with UPDATE_GOLDEN=1 to accept the new output:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn every_golden_file_has_a_case() {
    for entry in fs::read_dir(dir()).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if let Some(stem) = name
            .strip_suffix(".elrc")
            .or_else(|| name.strip_suffix(".trans.lrc"))
        {
            assert!(
                path.with_file_name(format!("{stem}.yrc")).exists(),
                "{} has no case",
                path.display()
            );
        }
    }
}
