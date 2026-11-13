//! Library view decisions (design-controls.md, "Library"): the album page's
//! facts and footer, and the level glyph on the playing row. Nothing here
//! sorts or filters library data; it only shows what the core returned.

use std::f64::consts::TAU;

use crate::logic::format;

/// Songs and whole minutes of a track list, for "12 songs, 48 minutes".
pub fn totals(durations: impl IntoIterator<Item = Option<u64>>) -> (u64, u64) {
    let (mut count, mut total_ms) = (0u64, 0u64);
    for d in durations {
        count += 1;
        total_ms = total_ms.saturating_add(d.unwrap_or(0));
    }
    (count, format::minutes(total_ms))
}

/// The genre an album page names: the one most of its songs have (the
/// first of equals, in track order), if any song has one.
pub fn main_genre<'a>(genres: impl IntoIterator<Item = Option<&'a str>>) -> Option<String> {
    let mut seen: Vec<(&str, usize)> = Vec::new();
    for g in genres
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|g| !g.is_empty())
    {
        match seen.iter_mut().find(|(name, _)| *name == g) {
            Some((_, n)) => *n += 1,
            None => seen.push((g, 1)),
        }
    }
    let best = seen.iter().map(|(_, n)| *n).max()?;
    seen.iter()
        .find(|(_, n)| *n == best)
        .map(|(g, _)| (*g).to_owned())
}

/// The level glyph's three bars standing still (reduced motion, paused).
pub const LEVEL_STILL: [f64; 3] = [0.45, 0.85, 0.6];

/// The level glyph's bar heights (0 to 1 of the glyph) `t` seconds into
/// playback: each bar mixes two slow waves at its own rates, so the three
/// move independently and never repeat in step.
pub fn level_bars(t: f64) -> [f64; 3] {
    const RATES: [(f64, f64, f64); 3] = [(1.7, 2.9, 0.0), (2.3, 1.3, 1.9), (1.1, 3.7, 4.1)];
    let mut out = [0.0; 3];
    for (bar, (a, b, phase)) in out.iter_mut().zip(RATES) {
        let wave = 0.6 * (TAU * a * t + phase).sin() + 0.4 * (TAU * b * t + 2.0 * phase).sin();
        *bar = (0.55 + 0.42 * wave).clamp(0.18, 1.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_count_songs_and_round_minutes() {
        assert_eq!(totals([Some(64_000), Some(32_000), None]), (3, 2));
        assert_eq!(totals(Vec::<Option<u64>>::new()), (0, 0));
        assert_eq!(totals([Some(10_000)]), (1, 1));
        assert_eq!(totals([Some(u64::MAX), Some(5)]).0, 2);
    }

    #[test]
    fn the_main_genre_is_the_commonest() {
        assert_eq!(
            main_genre([Some("Ambient"), Some("Jazz"), Some("Jazz")]),
            Some("Jazz".into())
        );
        assert_eq!(
            main_genre([Some("Ambient"), Some("Jazz")]),
            Some("Ambient".into())
        );
        assert_eq!(main_genre([None, Some(" "), None]), None);
        assert_eq!(main_genre([Some(" Folk ")]), Some("Folk".into()));
    }

    #[test]
    fn level_bars_move_within_the_glyph() {
        let mut changed = false;
        let first = level_bars(0.0);
        for i in 0..600 {
            let bars = level_bars(f64::from(i) / 60.0);
            for h in bars {
                assert!((0.18..=1.0).contains(&h), "{h}");
            }
            changed |= bars != first;
        }
        assert!(changed);
        // The bars do not move together.
        let a = level_bars(0.3);
        assert!(a[0] != a[1] && a[1] != a[2]);
        for h in LEVEL_STILL {
            assert!((0.18..=1.0).contains(&h));
        }
    }
}
