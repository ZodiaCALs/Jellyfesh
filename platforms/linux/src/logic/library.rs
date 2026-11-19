//! Library view decisions (design-controls.md, "Library"): the sort menus'
//! choices, the album page's facts and footer, and the level glyph on the
//! playing row. Nothing here sorts or filters library data: the menus name
//! one of the core's orders (`AlbumSort`, `TrackSort` with `descending`),
//! and the rest only shows what the core returned.

use std::f64::consts::TAU;

use jf_core::{AlbumSort, TrackSort};

use crate::logic::format;

/// The Albums sort menu, in its order (docs/ui.md, "Albums"), by menu key.
pub const ALBUM_SORTS: [(&str, AlbumSort); 4] = [
    ("title", AlbumSort::Title),
    ("artist", AlbumSort::Artist),
    ("year", AlbumSort::Year),
    ("added", AlbumSort::Added),
];

/// The Songs sort menu, in its order (docs/ui.md, "Songs"), by menu key.
pub const SONG_SORTS: [(&str, TrackSort); 5] = [
    ("title", TrackSort::Title),
    ("artist", TrackSort::Artist),
    ("album", TrackSort::Album),
    ("year", TrackSort::Year),
    ("added", TrackSort::Added),
];

/// The order Albums starts in, as in every app.
pub const ALBUMS_START: &str = "title";

/// The order Songs starts in: by artist, as in every app (docs/parity.md,
/// V6).
pub const SONGS_START: &str = "artist";

/// The core's album order for a menu key (Title for an unknown one).
pub fn album_sort(key: &str) -> AlbumSort {
    ALBUM_SORTS
        .iter()
        .find(|(k, _)| *k == key)
        .map_or(AlbumSort::Title, |(_, s)| *s)
}

/// The core's song order for a menu key (Title for an unknown one).
pub fn song_sort(key: &str) -> TrackSort {
    SONG_SORTS
        .iter()
        .find(|(k, _)| *k == key)
        .map_or(TrackSort::Title, |(_, s)| *s)
}

/// Whether choosing a sort turns Descending on: years and Recently Added
/// show the newest first; names read from A. The person can then flip it.
pub fn starts_descending(key: &str) -> bool {
    matches!(key, "year" | "added")
}

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
    fn sort_keys_name_the_cores_orders() {
        for (key, sort) in ALBUM_SORTS {
            assert_eq!(album_sort(key), sort);
        }
        for (key, sort) in SONG_SORTS {
            assert_eq!(song_sort(key), sort);
        }
        assert_eq!(album_sort("nonsense"), AlbumSort::Title);
        assert_eq!(song_sort("nonsense"), TrackSort::Title);
        let keys: Vec<&str> = SONG_SORTS.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, ["title", "artist", "album", "year", "added"]);
        let keys: Vec<&str> = ALBUM_SORTS.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, ["title", "artist", "year", "added"]);
    }

    #[test]
    fn albums_start_by_title_and_songs_by_artist() {
        assert_eq!(album_sort(ALBUMS_START), AlbumSort::Title);
        assert_eq!(song_sort(SONGS_START), TrackSort::Artist);
        assert!(!starts_descending(ALBUMS_START));
        assert!(!starts_descending(SONGS_START));
    }

    #[test]
    fn newest_first_for_years_and_recently_added() {
        assert!(starts_descending("year"));
        assert!(starts_descending("added"));
        assert!(!starts_descending("title"));
        assert!(!starts_descending("artist"));
        assert!(!starts_descending("album"));
    }

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
