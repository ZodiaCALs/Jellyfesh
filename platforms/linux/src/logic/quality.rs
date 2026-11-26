//! The quality badge and its menu (ADR-0026 §3; docs/ui.md, "The player bar
//! and the mini player"): what the badge says, the menu's format line, and
//! the levels it offers with the chosen one checked, or why there are none.
//! The levels and the note come from the core (`NowPlaying.qualities`,
//! `quality_note`); the chosen level is Streaming Quality in the settings.

use jf_core::{AudioFormat, NowPlaying, QualityLabel, QualityNote, StreamingQuality};

use crate::logic::format;

/// What the badge says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Badge {
    /// The playing quality's label, once the engine has opened the song.
    Label(QualityLabel),
    /// Before that: the chosen level.
    Level(StreamingQuality),
}

pub fn badge(np: &NowPlaying, chosen: StreamingQuality) -> Badge {
    match np.quality.as_ref().and_then(|q| q.quality) {
        Some(label) => Badge::Label(label),
        None => Badge::Level(chosen),
    }
}

/// The format line's parts ("FLAC · 24-bit · 96 kHz", "AAC · 256 kbit/s"):
/// the bit depth and sample rate for lossless audio, the bitrate otherwise.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    /// As people write it: FLAC, MP3, AAC, Opus.
    pub codec: Option<String>,
    pub bit_depth: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub bitrate_kbps: Option<u32>,
}

pub fn line(format: &AudioFormat) -> Line {
    let lossless = format.lossless.unwrap_or(matches!(
        format.quality,
        Some(QualityLabel::Lossless | QualityLabel::HiResLossless)
    ));
    let codec = format.codec.as_deref().map(format::codec);
    if lossless {
        Line {
            codec,
            bit_depth: format.bit_depth,
            sample_rate_hz: format.sample_rate_hz,
            bitrate_kbps: None,
        }
    } else {
        Line {
            codec,
            bitrate_kbps: format.bitrate_kbps,
            ..Line::default()
        }
    }
}

/// What the menu offers under the format line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choices {
    /// The levels in the menu's order (Auto, then the source's best first),
    /// each with whether it is the chosen one.
    Levels(Vec<(StreamingQuality, bool)>),
    /// "Files play as they are."
    FilesPlayAsTheyAre,
    /// "Quality is set in {service}."
    SetIn(String),
    /// No levels and no reason given: the format line alone.
    Nothing,
}

pub fn choices(np: &NowPlaying, chosen: StreamingQuality) -> Choices {
    if !np.qualities.is_empty() {
        return Choices::Levels(np.qualities.iter().map(|q| (*q, *q == chosen)).collect());
    }
    note_choices(np.quality_note.as_ref())
}

/// Why there are no levels, from a note: `NowPlaying.quality_note`, or
/// during remote playback the `Remote` slice's ("Quality is set in
/// {service}.").
pub fn note_choices(note: Option<&QualityNote>) -> Choices {
    match note {
        Some(QualityNote::FilesPlayAsTheyAre) => Choices::FilesPlayAsTheyAre,
        Some(QualityNote::SetInService { service }) => Choices::SetIn(service.clone()),
        None => Choices::Nothing,
    }
}

/// Every level, in Settings › Playback › Streaming's order.
pub const LEVELS: [StreamingQuality; 5] = [
    StreamingQuality::Auto,
    StreamingQuality::Lossless,
    StreamingQuality::High,
    StreamingQuality::Normal,
    StreamingQuality::Low,
];

/// A level's place in [`LEVELS`] (the Streaming Quality row's index).
pub fn level_index(level: StreamingQuality) -> u32 {
    LEVELS.iter().position(|l| *l == level).unwrap_or(0) as u32
}

/// The level at a place in [`LEVELS`]; Auto past the end.
pub fn level_at(index: u32) -> StreamingQuality {
    LEVELS
        .get(index as usize)
        .copied()
        .unwrap_or(StreamingQuality::Auto)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_core::TrackRef;

    fn playing() -> NowPlaying {
        NowPlaying {
            uid: 1,
            track: TrackRef::Library(1),
            path: None,
            title: "Song".into(),
            artist: None,
            album: None,
            duration_ms: None,
            artwork: None,
            format: None,
            link: None,
            quality: None,
            qualities: Vec::new(),
            quality_note: None,
            quality_level: None,
        }
    }

    fn audio(codec: &str, lossless: Option<bool>, quality: Option<QualityLabel>) -> AudioFormat {
        AudioFormat {
            codec: Some(codec.into()),
            sample_rate_hz: Some(96_000),
            bit_depth: Some(24),
            channels: Some(2),
            bitrate_kbps: Some(256),
            lossless,
            quality,
        }
    }

    #[test]
    fn the_badge_shows_the_label_once_known_else_the_chosen_level() {
        let mut np = playing();
        assert_eq!(
            badge(&np, StreamingQuality::High),
            Badge::Level(StreamingQuality::High)
        );
        np.quality = Some(audio("flac", Some(true), Some(QualityLabel::HiResLossless)));
        assert_eq!(
            badge(&np, StreamingQuality::High),
            Badge::Label(QualityLabel::HiResLossless)
        );
    }

    #[test]
    fn lossless_lines_give_depth_and_rate_lossy_ones_the_bitrate() {
        let flac = line(&audio("flac", Some(true), None));
        assert_eq!(flac.codec.as_deref(), Some("FLAC"));
        assert_eq!(
            (flac.bit_depth, flac.sample_rate_hz, flac.bitrate_kbps),
            (Some(24), Some(96_000), None)
        );
        let aac = line(&audio("aac", Some(false), None));
        assert_eq!(aac.codec.as_deref(), Some("AAC"));
        assert_eq!(
            (aac.bit_depth, aac.sample_rate_hz, aac.bitrate_kbps),
            (None, None, Some(256))
        );
        // Without the lossless flag, the label decides.
        let labelled = line(&audio("alac", None, Some(QualityLabel::Lossless)));
        assert_eq!(labelled.bitrate_kbps, None);
        let unlabelled = line(&audio("mp3", None, None));
        assert_eq!(unlabelled.bitrate_kbps, Some(256));
    }

    #[test]
    fn the_menu_offers_the_levels_with_the_chosen_one_checked_or_says_why_not() {
        let mut np = playing();
        assert_eq!(choices(&np, StreamingQuality::Auto), Choices::Nothing);
        np.quality_note = Some(QualityNote::FilesPlayAsTheyAre);
        assert_eq!(choices(&np, StreamingQuality::Auto), Choices::FilesPlayAsTheyAre);
        np.quality_note = Some(QualityNote::SetInService {
            service: "Spotify".into(),
        });
        assert_eq!(
            choices(&np, StreamingQuality::Auto),
            Choices::SetIn("Spotify".into())
        );
        np.qualities = vec![
            StreamingQuality::Auto,
            StreamingQuality::Lossless,
            StreamingQuality::High,
        ];
        np.quality_note = None;
        assert_eq!(
            choices(&np, StreamingQuality::High),
            Choices::Levels(vec![
                (StreamingQuality::Auto, false),
                (StreamingQuality::Lossless, false),
                (StreamingQuality::High, true),
            ])
        );
    }

    #[test]
    fn remote_playback_says_where_quality_is_set() {
        let note = QualityNote::SetInService {
            service: "Spotify".into(),
        };
        assert_eq!(note_choices(Some(&note)), Choices::SetIn("Spotify".into()));
        assert_eq!(note_choices(None), Choices::Nothing);
    }

    #[test]
    fn levels_round_trip_through_their_places() {
        for (i, level) in LEVELS.iter().enumerate() {
            assert_eq!(level_index(*level), i as u32);
            assert_eq!(level_at(i as u32), *level);
        }
        assert_eq!(level_at(99), StreamingQuality::Auto);
    }
}
