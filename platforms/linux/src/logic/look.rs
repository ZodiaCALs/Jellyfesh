//! The look (ADR-0020, docs/ui.md "The Look slice"): the core's resolved
//! values as the numbers and names the views draw with. With `customized`
//! false every value is the Luminous default, so the app draws exactly as
//! in v0.0.6.

use jf_core::{LookSnapshot, LyricsHighlight};

/// The colour of sung words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sung {
    /// The lyrics' sung ink (`lyrics.ink-sung`): v0.0.6, and Artwork.
    Ink,
    /// The accent text colour.
    Accent,
    /// The person's colour, `0xRRGGBB`.
    Rgb(u32),
}

/// What sung words are drawn in.
pub fn sung(look: &LookSnapshot) -> Sung {
    if !look.customized {
        return Sung::Ink;
    }
    match look.lyrics_highlight {
        LyricsHighlight::Artwork => Sung::Ink,
        LyricsHighlight::Accent => Sung::Accent,
        LyricsHighlight::Custom { rgb } => Sung::Rgb(rgb & 0xFF_FF_FF),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom() -> LookSnapshot {
        LookSnapshot {
            customized: true,
            ..LookSnapshot::defaults(1)
        }
    }

    #[test]
    fn not_customized_is_luminous() {
        let look = LookSnapshot {
            lyrics_highlight: LyricsHighlight::Accent,
            ..LookSnapshot::defaults(1)
        };
        assert_eq!(sung(&look), Sung::Ink);
    }

    #[test]
    fn the_highlight() {
        let mut look = custom();
        assert_eq!(sung(&look), Sung::Ink);
        look.lyrics_highlight = LyricsHighlight::Accent;
        assert_eq!(sung(&look), Sung::Accent);
        look.lyrics_highlight = LyricsHighlight::Custom { rgb: 0xFF12_3456 };
        assert_eq!(sung(&look), Sung::Rgb(0x12_3456));
    }
}
