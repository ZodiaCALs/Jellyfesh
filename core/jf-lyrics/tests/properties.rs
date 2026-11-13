//! Property tests: the binary-search cursor agrees with a linear scan, and the
//! parsers never panic and always produce well-formed documents.

use jf_lyrics::{
    Cursor, Direction, Format, Line, LyricDoc, Timing, Word, parse, parse_lrc, parse_ttml, parse_vtt,
};
use proptest::prelude::*;

/// The cursor rules written as the obvious linear scan.
fn linear_cursor(doc: &LyricDoc, t: u32) -> Cursor {
    let mut best: Option<usize> = None;
    for (i, line) in doc.lines.iter().enumerate() {
        let Some(start) = line.start_ms else { continue };
        // No end (or the u32::MAX sentinel) means the line never ends.
        let end = line.end_ms.filter(|&e| e != u32::MAX);
        let active = start <= t && end.is_none_or(|e| t < e);
        if active && best.is_none_or(|b| start >= doc.lines[b].start_ms.unwrap_or(0)) {
            best = Some(i);
        }
    }
    let Some(line) = best else {
        return Cursor::default();
    };
    let words = &doc.lines[line].words;
    let word = words.iter().rposition(|w| w.start_ms <= t);
    let word_progress = match word {
        None => 0.0,
        Some(j) if words[j].end_ms <= words[j].start_ms => 1.0,
        Some(j) => {
            let w = &words[j];
            ((t - w.start_ms) as f32 / (w.end_ms - w.start_ms) as f32).clamp(0.0, 1.0)
        }
    };
    Cursor {
        line: Some(line),
        word,
        word_progress,
    }
}

type LineSpec = (u32, Option<u32>, Vec<(u32, u32)>);

/// Sorted lines with random gaps, optional (possibly overlapping) ends and
/// words with random gaps and lengths, zero included.
fn sorted_doc(specs: Vec<LineSpec>) -> LyricDoc {
    let mut doc = LyricDoc::empty(Format::Lrc);
    doc.timing = Timing::Word;
    let mut t = 0u32;
    for (i, (gap, duration, words)) in specs.into_iter().enumerate() {
        t += gap;
        let mut w = t;
        let words = words
            .into_iter()
            .map(|(gap, len)| {
                w += gap;
                Word {
                    start_ms: w,
                    end_ms: w + len,
                    text: "w".into(),
                    joins_next: false,
                }
            })
            .collect();
        doc.lines.push(Line {
            id: i as u32,
            start_ms: Some(t),
            end_ms: duration.map(|d| t + d),
            agent: None,
            text: String::new(),
            words,
            background: Vec::new(),
            direction: Direction::Auto,
        });
    }
    doc
}

fn line_specs() -> impl Strategy<Value = Vec<LineSpec>> {
    prop::collection::vec(
        (
            0u32..1_500,
            prop::option::weighted(0.9, 0u32..4_000),
            prop::collection::vec((0u32..700, 0u32..900), 0..6),
        ),
        0..16,
    )
}

fn check_document(doc: &LyricDoc) {
    for (i, line) in doc.lines.iter().enumerate() {
        assert_eq!(line.id as usize, i);
        if let (Some(s), Some(e)) = (line.start_ms, line.end_ms) {
            assert!(e >= s);
        }
        if i > 0 {
            assert!(line.start_ms >= doc.lines[i - 1].start_ms);
        }
        assert_eq!(line.text, line.text.trim());
        for pair in line.words.windows(2) {
            assert!(pair[1].start_ms >= pair[0].start_ms);
        }
        assert!(
            line.words
                .iter()
                .all(|w| w.end_ms >= w.start_ms && !w.text.is_empty())
        );
    }
    let tl = doc.timeline();
    for t in [0, 1, 1_000, 60_000, u32::MAX] {
        assert_eq!(tl.cursor(t), linear_cursor(doc, t));
    }
}

/// What a view draws from the cursor: the line, the word and whether that
/// word is fully sung.
fn visible(c: Cursor) -> (Option<usize>, Option<usize>, bool) {
    (c.line, c.word, c.word_progress >= 1.0)
}

proptest! {
    #[test]
    fn next_change_is_the_first_visible_change(specs in line_specs(), probes in prop::collection::vec(0u32..40_000, 1..12)) {
        let doc = sorted_doc(specs);
        let tl = doc.timeline();
        for t in probes {
            let now = visible(tl.cursor(t));
            let next = tl.next_change(t);
            // Nothing visible changes before the reported time ...
            let until = next.unwrap_or(t.saturating_add(5_000)).min(t.saturating_add(5_000));
            for u in t + 1..until {
                prop_assert_eq!(visible(tl.cursor(u)), now, "t = {}, u = {}, next = {:?}", t, u, next);
            }
            // ... and something does at it.
            if let Some(n) = next {
                prop_assert!(n > t);
                prop_assert_ne!(visible(tl.cursor(n)), now, "t = {}, next = {}", t, n);
            }
        }
    }

    #[test]
    fn cursor_matches_a_linear_scan(specs in line_specs(), probes in prop::collection::vec(0u32..40_000, 0..24)) {
        let doc = sorted_doc(specs);
        let tl = doc.timeline();
        let mut times = probes;
        for line in &doc.lines {
            let mut edges = vec![line.start_ms.unwrap(), line.end_ms.unwrap_or(0)];
            edges.extend(line.words.iter().flat_map(|w| [w.start_ms, w.end_ms]));
            for e in edges {
                times.extend([e.saturating_sub(1), e, e.saturating_add(1)]);
            }
        }
        times.push(u32::MAX);
        for t in times {
            prop_assert_eq!(tl.cursor(t), linear_cursor(&doc, t), "t = {}", t);
        }
    }

    #[test]
    fn any_text_parses_without_panicking(input in "\\PC{0,200}") {
        let results = [parse(&input, None), parse_lrc(&input), parse_ttml(&input), parse_vtt(&input)];
        for doc in results.into_iter().flatten() {
            check_document(&doc);
        }
    }

    #[test]
    fn lrc_like_text_gives_well_formed_documents(
        lines in prop::collection::vec(
            (
                prop::collection::vec((0u32..3, 0u32..60, 0u32..1000), 0..3),
                "(v[0-9]: )?",
                prop::collection::vec(("[a-z]{0,6}|夜|空| ", prop::option::of((0u32..3, 0u32..60, 0u32..100))), 0..5),
            ),
            0..12,
        ),
        offset in -3_000i32..3_000,
    ) {
        let mut text = format!("[offset:{offset}]\n");
        for (stamps, voice, words) in lines {
            for (m, s, ms) in stamps {
                text.push_str(&format!("[{m:02}:{s:02}.{ms:03}]"));
            }
            text.push_str(&voice);
            for (word, stamp) in words {
                if let Some((m, s, cs)) = stamp {
                    text.push_str(&format!("<{m:02}:{s:02}.{cs:02}>"));
                }
                text.push_str(&word);
            }
            text.push('\n');
        }
        let doc = parse_lrc(&text).unwrap();
        check_document(&doc);
    }

    #[test]
    fn ttml_like_text_gives_well_formed_documents(
        ps in prop::collection::vec(
            (
                prop::option::of(0u32..20_000),
                prop::option::of(0u32..20_000),
                prop::collection::vec((prop::option::of((0u32..20_000, 0u32..2_000)), "[a-z]{1,5}|星|x-bg", " ?"), 0..5),
            ),
            0..8,
        ),
    ) {
        let mut text = String::from(r#"<tt xmlns:ttm="http://www.w3.org/ns/ttml#metadata"><body><div>"#);
        for (begin, end, spans) in ps {
            text.push_str("<p");
            if let Some(b) = begin {
                text.push_str(&format!(r#" begin="{b}ms""#));
            }
            if let Some(e) = end {
                text.push_str(&format!(r#" end="{e}ms""#));
            }
            text.push('>');
            for (time, word, space) in spans {
                let role = if word == "x-bg" { r#" ttm:role="x-bg""# } else { "" };
                match time {
                    Some((b, d)) => text.push_str(&format!(r#"<span{role} begin="{b}ms" dur="{d}ms">{word}</span>{space}"#)),
                    None => text.push_str(&format!("<span{role}>{word}</span>{space}")),
                }
            }
            text.push_str("</p>");
        }
        text.push_str("</div></body></tt>");
        let doc = parse_ttml(&text).unwrap();
        check_document(&doc);
    }
}
