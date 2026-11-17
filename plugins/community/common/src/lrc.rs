//! LRC lyrics as both services send them: read, merge a translation in,
//! write back.
//!
//! Jellyfesh reads two lines with the same time stamp as a line and its
//! translation, so a translation is merged in as a second line right after
//! the line it translates, with the same stamp.

/// LRC lyrics, read.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lrc {
    /// ID tags (`ti`, `ar`, `al`, `by`, `offset`, …) in the order found.
    pub tags: Vec<(String, String)>,
    /// Timed lines, in time order (lines with the same time keep their order).
    pub lines: Vec<Line>,
    /// Lines without a time stamp, in order.
    pub plain: Vec<String>,
}

/// One timed line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// When the line starts, in milliseconds.
    pub time_ms: u64,
    /// Its text (may be empty: an instrumental gap).
    pub text: String,
}

impl Lrc {
    /// Whether there is at least one timed line.
    pub fn is_synced(&self) -> bool {
        !self.lines.is_empty()
    }

    /// Whether there is no text at all.
    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|l| l.text.trim().is_empty()) && self.plain.iter().all(|l| l.trim().is_empty())
    }

    /// The value of the ID tag `key` (case-insensitive).
    pub fn tag(&self, key: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }
}

/// Reads LRC text. Lines that are neither tags nor timed are kept as plain
/// lines; nothing is an error.
pub fn parse(text: &str) -> Lrc {
    let mut lrc = Lrc::default();
    for raw in text.lines() {
        let line = raw.trim_end_matches('\r').trim();
        if line.is_empty() {
            continue;
        }
        let mut rest = line;
        let mut stamps = Vec::new();
        let mut tag = None;
        while let Some(inner) = rest.strip_prefix('[') {
            let Some(end) = inner.find(']') else {
                break;
            };
            let content = &inner[..end];
            if let Some(ms) = parse_time(content) {
                stamps.push(ms);
                rest = &inner[end + 1..];
            } else if stamps.is_empty()
                && let Some((key, value)) = parse_tag(content)
            {
                tag = Some((key, value));
                rest = &inner[end + 1..];
                break;
            } else {
                break;
            }
        }
        if !stamps.is_empty() {
            let text = rest.trim().to_string();
            for ms in stamps {
                lrc.lines.push(Line {
                    time_ms: ms,
                    text: text.clone(),
                });
            }
        } else if let Some((key, value)) = tag.filter(|_| rest.trim().is_empty()) {
            lrc.tags.push((key, value));
        } else {
            lrc.plain.push(line.to_string());
        }
    }
    lrc.lines.sort_by_key(|l| l.time_ms);
    lrc
}

/// `mm:ss`, `mm:ss.f`, `mm:ss.ff`, `mm:ss.fff` or `mm:ss:ff` → milliseconds.
pub fn parse_time(text: &str) -> Option<u64> {
    let (minutes, rest) = text.split_once(':')?;
    if minutes.is_empty() || minutes.len() > 4 || !minutes.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (seconds, fraction) = match rest.split_once(['.', ':']) {
        Some((s, f)) => (s, f),
        None => (rest, ""),
    };
    if seconds.is_empty() || seconds.len() > 2 || !seconds.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if fraction.len() > 3 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let minutes: u64 = minutes.parse().ok()?;
    let seconds: u64 = seconds.parse().ok()?;
    if seconds >= 60 {
        return None;
    }
    let fraction_ms = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u64>().ok()? * 100,
        2 => fraction.parse::<u64>().ok()? * 10,
        _ => fraction.parse::<u64>().ok()?,
    };
    Some((minutes * 60 + seconds) * 1000 + fraction_ms)
}

/// `[key:value]` with an ASCII-letter key.
fn parse_tag(content: &str) -> Option<(String, String)> {
    let (key, value) = content.split_once(':')?;
    let key = key.trim();
    if key.is_empty() || key.len() > 16 || !key.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    Some((key.to_ascii_lowercase(), value.trim().to_string()))
}

/// Milliseconds → `mm:ss.xx` (or `mm:ss.xxx` when the time is not a whole
/// number of hundredths).
pub fn format_time(ms: u64) -> String {
    let minutes = ms / 60_000;
    let seconds = (ms / 1000) % 60;
    let fraction = ms % 1000;
    if fraction.is_multiple_of(10) {
        format!("{minutes:02}:{seconds:02}.{:02}", fraction / 10)
    } else {
        format!("{minutes:02}:{seconds:02}.{fraction:03}")
    }
}

/// Writes LRC text: the tags, then the timed lines (then any plain lines,
/// for unsynchronised lyrics).
pub fn render(lrc: &Lrc) -> String {
    let mut out = String::new();
    for (key, value) in &lrc.tags {
        out.push_str(&format!("[{key}:{value}]\n"));
    }
    for line in &lrc.lines {
        out.push_str(&format!("[{}]{}\n", format_time(line.time_ms), line.text));
    }
    if lrc.lines.is_empty() {
        for line in &lrc.plain {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// `original` with each line of `translation` placed right after the line
/// it translates (the same time stamp). Translation lines that match no
/// line, are empty, say only `//` (the services' "no translation here") or
/// repeat the line are left out.
pub fn with_translation(original: &Lrc, translation: &Lrc) -> Lrc {
    let mut used = vec![false; translation.lines.len()];
    let mut lines = Vec::with_capacity(original.lines.len() * 2);
    for line in &original.lines {
        lines.push(line.clone());
        if line.text.trim().is_empty() {
            continue;
        }
        let found = translation
            .lines
            .iter()
            .enumerate()
            .find(|(i, t)| !used[*i] && t.time_ms == line.time_ms)
            .map(|(i, t)| (i, t.text.trim()));
        if let Some((i, text)) = found {
            used[i] = true;
            if !text.is_empty() && text != "//" && text != line.text.trim() {
                lines.push(Line {
                    time_ms: line.time_ms,
                    text: text.to_string(),
                });
            }
        }
    }
    Lrc {
        tags: original.tags.clone(),
        lines,
        plain: original.plain.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_times() {
        assert_eq!(parse_time("00:12.34"), Some(12_340));
        assert_eq!(parse_time("01:02"), Some(62_000));
        assert_eq!(parse_time("1:02.5"), Some(62_500));
        assert_eq!(parse_time("10:00.123"), Some(600_123));
        assert_eq!(parse_time("00:01:50"), Some(1_500));
        assert_eq!(parse_time("100:00.00"), Some(6_000_000));
        for bad in ["ti:x", "00:61.00", "00:1x", "", ":12", "00:12.3456"] {
            assert_eq!(parse_time(bad), None, "{bad}");
        }
    }

    #[test]
    fn reads_tags_lines_and_multiple_stamps() {
        let lrc = parse(
            "[ti:Mock Title]\r\n[ar:Mock Artist]\n[offset:+100]\n[00:05.00][00:15.00]Chorus\n\
             [00:10.00]Verse\n[00:20.00]\nnot timed\n",
        );
        assert_eq!(lrc.tag("TI"), Some("Mock Title"));
        assert_eq!(lrc.tag("offset"), Some("+100"));
        let times: Vec<(u64, &str)> = lrc.lines.iter().map(|l| (l.time_ms, l.text.as_str())).collect();
        assert_eq!(
            times,
            vec![
                (5_000, "Chorus"),
                (10_000, "Verse"),
                (15_000, "Chorus"),
                (20_000, "")
            ]
        );
        assert_eq!(lrc.plain, vec!["not timed".to_string()]);
        assert!(lrc.is_synced() && !lrc.is_empty());
    }

    #[test]
    fn writes_what_it_read() {
        let text = "[ti:Mock]\n[00:01.50]One\n[00:02.125]Two\n[01:00.00]\n";
        assert_eq!(render(&parse(text)), text);
        assert_eq!(format_time(61_005), "01:01.005");
        assert_eq!(
            render(&parse("just words\nmore words")),
            "just words\nmore words\n"
        );
    }

    #[test]
    fn merges_translations_as_bilingual_lines() {
        let original = parse("[ti:T]\n[00:01.00]第一行\n[00:02.00]第二行\n[00:03.00]\n[00:04.00]第四行\n");
        let translation = parse(
            "[by:someone]\n[00:01.00]First line\n[00:02.00]//\n[00:03.00]gap\n[00:04.00]第四行\n[00:09.00]stray\n",
        );
        let merged = render(&with_translation(&original, &translation));
        assert_eq!(
            merged,
            "[ti:T]\n[00:01.00]第一行\n[00:01.00]First line\n[00:02.00]第二行\n[00:03.00]\n[00:04.00]第四行\n"
        );
    }

    #[test]
    fn empty_lyrics_are_empty() {
        assert!(parse("").is_empty());
        assert!(parse("[ti:x]\n[00:01.00]\n").is_empty());
        assert!(!parse("[00:01.00]a").is_empty());
    }
}
