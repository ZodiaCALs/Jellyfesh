//! Formatting and parsing helpers for terminal output.

/// `println!` that exits quietly when stdout is closed (`jf ls | head`)
/// instead of panicking.
macro_rules! outln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        if let Err(e) = writeln!(std::io::stdout(), $($arg)*) {
            if e.kind() == std::io::ErrorKind::BrokenPipe {
                std::process::exit(0);
            }
        }
    }};
}

use std::io::IsTerminal;

/// Formats milliseconds as `m:ss` or `h:mm:ss`.
pub fn fmt_ms(ms: u64) -> String {
    let s = ms / 1_000;
    let (h, m, s) = (s / 3_600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Formats milliseconds as `mm:ss.cc` (lyric timestamps).
pub fn fmt_stamp(ms: u32) -> String {
    let cs = (ms % 1_000) / 10;
    let s = ms / 1_000;
    format!("{:02}:{:02}.{cs:02}", s / 60, s % 60)
}

/// Parses `83`, `83.5`, `1:23`, `1:23.45` or `1:02:03.5` into milliseconds.
pub fn parse_time(text: &str) -> Result<u64, String> {
    let bad = || format!("expected a time like 1:23 or 83.5, got {text:?}");
    let parts: Vec<&str> = text.trim().split(':').collect();
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
        return Err(bad());
    }
    let (whole, last) = parts.split_at(parts.len() - 1);
    let seconds: f64 = last[0].parse().map_err(|_| bad())?;
    if !seconds.is_finite() || seconds < 0.0 || (!whole.is_empty() && seconds >= 60.0) {
        return Err(bad());
    }
    // Hours and minutes, folded into whole minutes.
    let mut minutes = 0u64;
    for (i, p) in whole.iter().enumerate() {
        let v: u64 = p.parse().map_err(|_| bad())?;
        if i > 0 && v >= 60 {
            return Err(bad());
        }
        minutes = minutes * 60 + v;
    }
    let ms = (minutes as f64 * 60.0 + seconds) * 1_000.0;
    Ok(ms.round() as u64)
}

/// True when stdout is a terminal that accepts colour (honours `NO_COLOR`).
pub fn colour() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
}

/// A coloured swatch: two spaces with an sRGB background.
pub fn swatch(rgba: [u8; 4]) -> String {
    if colour() {
        format!("\x1b[48;2;{};{};{}m    \x1b[0m", rgba[0], rgba[1], rgba[2])
    } else {
        String::new()
    }
}

pub fn hex(rgba: [u8; 4]) -> String {
    if rgba[3] == 255 {
        format!("#{:02x}{:02x}{:02x}", rgba[0], rgba[1], rgba[2])
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", rgba[0], rgba[1], rgba[2], rgba[3])
    }
}

pub fn bold(s: &str) -> String {
    if colour() {
        format!("\x1b[1m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn dim(s: &str) -> String {
    if colour() {
        format!("\x1b[2m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

/// Terminal columns a character takes: 2 for East Asian wide and full-width
/// characters and most emoji, 0 for combining marks and zero-width
/// characters, 1 otherwise. An approximation of UAX #11, enough to fit lyric
/// lines to the terminal.
pub fn display_width(c: char) -> usize {
    let cp = c as u32;
    let zero = [
        0x0300..=0x036F,
        0x0483..=0x0489,
        0x0591..=0x05BD,
        0x0610..=0x061A,
        0x064B..=0x065F,
        0x1AB0..=0x1AFF,
        0x1DC0..=0x1DFF,
        0x200B..=0x200F,
        0x20D0..=0x20FF,
        0xFE00..=0xFE0F,
        0xFE20..=0xFE2F,
    ];
    if c.is_control() || zero.iter().any(|r| r.contains(&cp)) {
        return 0;
    }
    let wide = [
        0x1100..=0x115F,
        0x2E80..=0x303E,
        0x3041..=0x33FF,
        0x3400..=0x4DBF,
        0x4E00..=0x9FFF,
        0xA000..=0xA4CF,
        0xAC00..=0xD7A3,
        0xF900..=0xFAFF,
        0xFE30..=0xFE4F,
        0xFF00..=0xFF60,
        0xFFE0..=0xFFE6,
        0x1F300..=0x1F64F,
        0x1F900..=0x1F9FF,
        0x20000..=0x3FFFD,
    ];
    if wide.iter().any(|r| r.contains(&cp)) {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_times() {
        assert_eq!(parse_time("83"), Ok(83_000));
        assert_eq!(parse_time("83.5"), Ok(83_500));
        assert_eq!(parse_time("1:23"), Ok(83_000));
        assert_eq!(parse_time("1:23.45"), Ok(83_450));
        assert_eq!(parse_time("1:02:03.5"), Ok(3_723_500));
        assert!(parse_time("1:60").is_err());
        assert!(parse_time("1:60:00").is_err());
        assert!(parse_time("").is_err());
        assert!(parse_time("a:b").is_err());
        assert!(parse_time("-3").is_err());
    }

    #[test]
    fn measures_display_width() {
        let w = |s: &str| s.chars().map(display_width).sum::<usize>();
        assert_eq!(w("abc"), 3);
        assert_eq!(w("夜に駆ける"), 10);
        assert_eq!(w("한국어"), 6);
        assert_eq!(w("e\u{301}"), 1);
        assert_eq!(w("ＡＢ"), 4);
    }

    #[test]
    fn formats_times() {
        assert_eq!(fmt_ms(0), "0:00");
        assert_eq!(fmt_ms(83_999), "1:23");
        assert_eq!(fmt_ms(3_723_000), "1:02:03");
        assert_eq!(fmt_stamp(83_456), "01:23.45");
    }
}
