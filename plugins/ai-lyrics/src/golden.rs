//! Golden tests for alignment and line making (ADR-0019 §6, Tests).
//!
//! Every `tests/golden/<name>.json` is a case of invented test data: plain
//! lyrics (`lyrics`, lines; absent for line making) and what a recogniser
//! heard (`segments` with `words` as `[text, start_ms, end_ms]`). Its
//! enhanced LRC is compared with `tests/golden/<name>.lrc`, and must read
//! back through the core's lyrics parser with every word timed. A case
//! that must fail names the failure in `fails` instead, and has no `.lrc`.
//!
//! A mismatch fails with both texts. To accept new output, run
//! `UPDATE_GOLDEN=1 cargo test -p jf-ai-lyrics golden` and review the
//! changed `.lrc` files like any other change.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::model::{Segment, Transcript, Word};
use crate::{align, elrc, lines};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Says what the case is about, and that it is invented.
    #[serde(rename = "_comment")]
    _comment: String,
    #[serde(default)]
    duration_ms: Option<u32>,
    #[serde(default)]
    lyrics: Option<Vec<String>>,
    segments: Vec<CaseSegment>,
    /// `NoLyrics`, `NothingHeard` or `NoMatch`.
    #[serde(default)]
    fails: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseSegment {
    #[serde(default)]
    text: String,
    #[serde(default)]
    start_ms: Option<u32>,
    #[serde(default)]
    end_ms: Option<u32>,
    #[serde(default)]
    words: Vec<(String, u32, u32)>,
}

impl Case {
    fn transcript(&self) -> Transcript {
        Transcript {
            language: None,
            segments: self
                .segments
                .iter()
                .map(|s| {
                    let words: Vec<Word> = s
                        .words
                        .iter()
                        .map(|(text, start_ms, end_ms)| Word {
                            text: text.clone(),
                            start_ms: *start_ms,
                            end_ms: *end_ms,
                        })
                        .collect();
                    Segment {
                        text: s.text.clone(),
                        start_ms: s.start_ms.or(words.first().map(|w| w.start_ms)).unwrap_or(0),
                        end_ms: s.end_ms.or(words.last().map(|w| w.end_ms)).unwrap_or(0),
                        words,
                    }
                })
                .collect(),
        }
    }

    /// The enhanced LRC, or the failure's name.
    fn run(&self) -> Result<String, String> {
        let transcript = self.transcript();
        let made = match &self.lyrics {
            Some(lyrics) => align::align(&lyrics.join("\n"), &transcript, self.duration_ms)
                .map_err(|failure| format!("{failure:?}"))?,
            None => lines::lines(&transcript),
        };
        Ok(elrc::render(&made))
    }
}

fn cases() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    files
}

/// The core reads the text as enhanced LRC with every line that has words
/// timed word by word, in order.
fn check_reads_back(name: &str, text: &str) {
    let doc = jf_lyrics::parse(text, Some(jf_lyrics::Format::EnhancedLrc))
        .unwrap_or_else(|e| panic!("{name}: the core cannot read it: {e}"));
    assert_eq!(doc.timing, jf_lyrics::Timing::Word, "{name}: word timing");
    let mut last = 0;
    for line in &doc.lines {
        if line.text.is_empty() {
            assert!(line.words.is_empty(), "{name}: a pause has no words");
            continue;
        }
        assert!(
            !line.words.is_empty(),
            "{name}: {:?} has no word times",
            line.text
        );
        for word in &line.words {
            assert!(
                word.start_ms >= last,
                "{name}: {:?} starts before the word before it",
                word.text
            );
            assert!(word.end_ms >= word.start_ms);
            last = word.start_ms;
        }
    }
}

#[test]
fn golden_files_match() {
    let update = std::env::var("UPDATE_GOLDEN").is_ok_and(|v| v == "1");
    let mut failures = Vec::new();
    let files = cases();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = fs::read_to_string(path).unwrap();
        let case: Case = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let golden = path.with_extension("lrc");
        match (case.run(), &case.fails) {
            (Err(failure), Some(expected)) => {
                if failure != *expected {
                    failures.push(format!("{name}: failed with {failure}, expected {expected}"));
                }
                if golden.exists() {
                    failures.push(format!("{name}: a failing case has no .lrc"));
                }
            }
            (Err(failure), None) => failures.push(format!("{name}: failed with {failure}")),
            (Ok(_), Some(expected)) => failures.push(format!("{name}: expected {expected}")),
            (Ok(made), None) => {
                check_reads_back(&name, &made);
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
        }
    }
    assert!(
        files.len() >= 12,
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
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "lrc") {
            assert!(
                path.with_extension("json").exists(),
                "{} has no case",
                path.display()
            );
        }
    }
}
