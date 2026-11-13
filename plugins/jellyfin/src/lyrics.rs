//! Jellyfin lyrics (`/Audio/{id}/Lyrics`, Jellyfin 10.9 and later) as LRC or
//! plain text.
//!
//! * Lines with a `Start` (100-nanosecond ticks) become LRC with one
//!   `[mm:ss.xxx]` stamp per line, rounded to the millisecond. A line
//!   without a start in synced lyrics keeps the previous line's time.
//! * `Metadata.Offset` ("the lyric offset compared to audio in ticks") is
//!   applied like LRC's `[offset:]`: positive means the lyrics appear
//!   sooner. Stamps stop at zero.
//! * Lyrics without any start time become plain text, one line per line.

use jf_plugin_sdk::prelude::*;

use crate::items::TICKS_PER_MS;
use crate::model::Lyrics;

/// The payload for Jellyfin lyrics, or none when they have no text.
pub fn payload(lyrics: &Lyrics) -> Option<LyricPayload> {
    if lyrics.lyrics.iter().all(|line| line.text.trim().is_empty()) {
        return None;
    }
    let synced = lyrics.lyrics.iter().any(|line| line.start.is_some());
    Some(if synced {
        LyricPayload::new(LyricFormat::Lrc, lrc(lyrics))
    } else {
        LyricPayload::new(LyricFormat::Plain, plain(lyrics))
    })
}

fn lrc(lyrics: &Lyrics) -> String {
    let offset = lyrics.metadata.offset.unwrap_or(0);
    let mut out = String::new();
    let mut last = 0i64;
    for line in &lyrics.lyrics {
        let start = line.start.unwrap_or(last);
        last = start;
        let ticks = start.saturating_sub(offset).max(0);
        // Round to the nearest millisecond.
        let ms = (ticks + TICKS_PER_MS / 2) / TICKS_PER_MS;
        out.push_str(&stamp(u64::try_from(ms).unwrap_or(0)));
        out.push_str(&one_line(&line.text));
        out.push('\n');
    }
    out
}

fn plain(lyrics: &Lyrics) -> String {
    let lines: Vec<String> = lyrics.lyrics.iter().map(|l| one_line(&l.text)).collect();
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

fn one_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

/// `[mm:ss.xxx]`; minutes go past 59 as LRC allows.
pub fn stamp(ms: u64) -> String {
    format!("[{:02}:{:02}.{:03}]", ms / 60_000, (ms / 1000) % 60, ms % 1000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LyricLine, LyricMetadata};

    fn line(start: Option<i64>, text: &str) -> LyricLine {
        LyricLine {
            text: text.into(),
            start,
        }
    }

    #[test]
    fn ticks_become_lrc() {
        let lyrics = Lyrics {
            metadata: LyricMetadata {
                offset: Some(-1_500_000),
                is_synced: Some(true),
            },
            lyrics: vec![
                line(Some(0), "First"),
                line(Some(12_345_678), "Second\r\nline"),
                line(None, "No time"),
                line(Some(36_000_000_000), "An hour"),
            ],
        };
        let payload = payload(&lyrics).unwrap();
        assert_eq!(payload.format, LyricFormat::Lrc);
        assert_eq!(
            payload.text,
            "[00:00.150]First\n[00:01.385]Second line\n[00:01.385]No time\n[60:00.150]An hour\n"
        );
    }

    #[test]
    fn untimed_lyrics_become_plain_text() {
        let lyrics = Lyrics {
            metadata: LyricMetadata::default(),
            lyrics: vec![line(None, "One"), line(None, ""), line(None, "Two")],
        };
        let payload = payload(&lyrics).unwrap();
        assert_eq!(payload.format, LyricFormat::Plain);
        assert_eq!(payload.text, "One\n\nTwo\n");
        let empty = Lyrics {
            metadata: LyricMetadata::default(),
            lyrics: vec![line(Some(0), " ")],
        };
        assert!(super::payload(&empty).is_none());
    }
}
