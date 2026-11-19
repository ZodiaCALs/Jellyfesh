//! The lyrics bar and Desktop Lyrics (ADR-0022 §6, docs/ui.md "Lyrics, word
//! by word"): one line of the lyrics (and its translation when Show
//! Translations is on), filling word by word in the highlight colour. What
//! shows when comes from the core's timing helper (`jf_core::lyrics_bar`);
//! this turns its answer into what the line widget draws.

use jf_core::LyricsBar;

/// Whether the player shows the lyrics bar: the setting is on, the engine
/// plays (a remote session has its own lyrics) and the lyrics are timed.
pub fn shown(setting: bool, remote: bool, synced: bool) -> bool {
    setting && !remote && synced
}

/// The sweep the line widget draws from each word's progress (0 to come,
/// 1 sung): the word being sung (every word before it lit) and how much of
/// it, or the last word whole once the line is sung. `None` without word
/// timing.
pub fn sweep(words: &[f32]) -> Option<(usize, f32)> {
    if words.is_empty() {
        return None;
    }
    match words.iter().position(|w| *w < 1.0) {
        Some(i) => Some((i, words[i].clamp(0.0, 1.0))),
        None => Some((words.len() - 1, 1.0)),
    }
}

/// Whether a word is being sung now: the bar is then drawn every frame.
pub fn sweeping(bar: &LyricsBar) -> bool {
    bar.words.iter().any(|w| *w > 0.0 && *w < 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_shows_for_timed_lyrics_on_this_device() {
        assert!(shown(true, false, true));
        assert!(!shown(false, false, true));
        assert!(!shown(true, true, true));
        assert!(!shown(true, false, false));
    }

    #[test]
    fn the_sweep_follows_the_words() {
        assert_eq!(sweep(&[]), None);
        assert_eq!(sweep(&[0.0, 0.0]), Some((0, 0.0)));
        assert_eq!(sweep(&[1.0, 0.25, 0.0]), Some((1, 0.25)));
        assert_eq!(sweep(&[1.0, 1.0, 0.0]), Some((2, 0.0)));
        assert_eq!(sweep(&[1.0, 1.0]), Some((1, 1.0)));
    }

    #[test]
    fn frames_only_while_a_word_is_sung() {
        let bar = |words: Vec<f32>| LyricsBar {
            line: Some(0),
            next: None,
            words,
            line_progress: 0.0,
        };
        assert!(sweeping(&bar(vec![1.0, 0.5, 0.0])));
        assert!(!sweeping(&bar(vec![1.0, 0.0])));
        assert!(!sweeping(&bar(vec![])));
    }
}
