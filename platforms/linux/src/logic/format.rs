//! Text for times and stream facts. Words are translated by the UI; these
//! helpers only produce numbers and separators that read the same in every
//! launch language.

/// `m:ss`, or `h:mm:ss` from one hour, for a position or duration in ms.
pub fn clock(ms: u64) -> String {
    let total = ms / 1_000;
    let (h, m, s) = (total / 3_600, (total / 60) % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// The time left with a minus sign (U+2212), `−1:23`, in whole seconds of
/// the length less whole seconds played, so the two times always add up to
/// the length and both change at the same moment.
pub fn remaining(position_ms: u64, duration_ms: u64) -> String {
    let left = (duration_ms / 1_000).saturating_sub(position_ms / 1_000);
    format!("\u{2212}{}", clock(left * 1_000))
}

/// A remaining time as wide as any for a track of `duration_ms` (in
/// tabular figures every digit is as wide as 8), so a seek bar can keep its
/// line still while the times change.
pub fn widest_remaining(duration_ms: u64) -> String {
    let secs = duration_ms / 1_000;
    let digits = if secs >= 36_000 {
        "88:88:88"
    } else if secs >= 3_600 {
        "8:88:88"
    } else if secs >= 600 {
        "88:88"
    } else {
        "8:88"
    };
    format!("\u{2212}{digits}")
}

/// A position in microseconds as whole milliseconds, never negative.
pub fn us_to_ms(us: i64) -> u64 {
    u64::try_from(us.max(0) / 1_000).unwrap_or(0)
}

/// Whole minutes for "42 min", rounded to the nearest minute but at least 1
/// for anything that has a length.
pub fn minutes(ms: u64) -> u64 {
    if ms == 0 {
        0
    } else {
        (ms.saturating_add(30_000) / 60_000).max(1)
    }
}

/// A sample rate for people: `44.1 kHz`, `48 kHz`, `192 kHz`.
pub fn sample_rate(hz: u32) -> String {
    let khz = f64::from(hz) / 1_000.0;
    if (khz - khz.round()).abs() < 0.05 {
        format!("{} kHz", khz.round() as u32)
    } else {
        format!("{khz:.1} kHz")
    }
}

/// The codec name as people know it (`flac` → `FLAC`, `pcm_s16le` → `PCM`).
pub fn codec(name: &str) -> String {
    let base = name.split('_').next().unwrap_or(name);
    match base {
        "vorbis" => "Vorbis".into(),
        "opus" => "Opus".into(),
        "alac" => "ALAC".into(),
        other => other.to_ascii_uppercase(),
    }
}

/// A count with its thousands grouped, as every launch language writes
/// them: `1,204`.
pub fn count(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Joins the parts that are present with a middle dot: `Artist · Album`.
pub fn dot_join<'a>(parts: impl IntoIterator<Item = Option<&'a str>>) -> String {
    parts
        .into_iter()
        .flatten()
        .filter(|p| !p.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Up to two initials for an avatar: "Placeholder Artist" → "PA",
/// "坂本" → "坂".
pub fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let mut out = String::new();
    for word in words.iter().take(2) {
        if let Some(c) = word.chars().find(|c| c.is_alphanumeric()) {
            if !c.is_ascii() && !out.is_empty() {
                break;
            }
            out.extend(c.to_uppercase());
            if !c.is_ascii() {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(84_999), "1:24");
        assert_eq!(clock(3_599_000), "59:59");
        assert_eq!(clock(3_600_000), "1:00:00");
        assert_eq!(clock(3_725_000), "1:02:05");
    }

    #[test]
    fn remaining_times_add_up_to_the_length() {
        assert_eq!(remaining(19_600, 64_000), "\u{2212}0:45");
        assert_eq!(remaining(19_600, 64_300), "\u{2212}0:45");
        assert_eq!(remaining(0, 3_725_000), "\u{2212}1:02:05");
        assert_eq!(remaining(70_000, 64_000), "\u{2212}0:00");
        assert_eq!(widest_remaining(64_000), "\u{2212}8:88");
        assert_eq!(widest_remaining(600_000), "\u{2212}88:88");
        assert_eq!(widest_remaining(3_725_000), "\u{2212}8:88:88");
        assert_eq!(widest_remaining(40_000_000), "\u{2212}88:88:88");
    }

    #[test]
    fn microseconds_and_minutes() {
        assert_eq!(us_to_ms(-5), 0);
        assert_eq!(us_to_ms(1_999), 1);
        assert_eq!(minutes(0), 0);
        assert_eq!(minutes(10_000), 1);
        assert_eq!(minutes(2_520_000), 42);
        assert_eq!(minutes(2_549_000), 42);
        assert_eq!(minutes(2_551_000), 43);
    }

    #[test]
    fn stream_facts() {
        assert_eq!(sample_rate(44_100), "44.1 kHz");
        assert_eq!(sample_rate(48_000), "48 kHz");
        assert_eq!(sample_rate(192_000), "192 kHz");
        assert_eq!(codec("flac"), "FLAC");
        assert_eq!(codec("pcm_s16le"), "PCM");
        assert_eq!(codec("vorbis"), "Vorbis");
    }

    #[test]
    fn counts_group_thousands() {
        assert_eq!(count(0), "0");
        assert_eq!(count(999), "999");
        assert_eq!(count(1_204), "1,204");
        assert_eq!(count(96_000), "96,000");
        assert_eq!(count(1_234_567), "1,234,567");
    }

    #[test]
    fn joins_and_initials() {
        assert_eq!(dot_join([Some("A"), None, Some("B")]), "A · B");
        assert_eq!(dot_join([Some(" "), Some("B")]), "B");
        assert_eq!(dot_join([None, None]), "");
        assert_eq!(initials("Placeholder Artist"), "PA");
        assert_eq!(initials("the tide"), "TT");
        assert_eq!(initials("坂本 龍"), "坂");
        assert_eq!(initials("(Mono)"), "M");
        assert_eq!(initials(""), "");
    }
}
