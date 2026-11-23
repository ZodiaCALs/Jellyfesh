//! Playback rules: what the core and the engine decide, for a player whose
//! audio is the browser's.
//!
//! `jf-core` and `jf-engine` do not build for the browser (threads, cpal,
//! libopus, ureq), so the rules they apply are mirrored here, with the same
//! numbers and the same `serde` shapes. The host-side tests of this crate
//! compare the mirrors with the originals (`tests/mirrors.rs`), so they
//! cannot drift apart unnoticed.
//!
//! | Rule | Original |
//! | --- | --- |
//! | Previous restarts the track after 3 s | `jf-core` actor, `RESTART_THRESHOLD_MS` |
//! | A play counts at its end, or after half the track (at most 4 min) | `jf-core` actor, `finish_play` |
//! | Album key for gapless joins inside one album | `jf-core` actor, `album_key` |
//! | Crossfade: 1–12 s, never more than half of either track, at least 50 ms, not inside one album with continuity on, equal-power | `jf-engine` `CrossfadeSettings`, `compute_plan`, `Fade` |
//! | Ten one-octave bands at the ISO frequencies, Q 1.41, the presets | `jf-engine` `EqSettings` |
//! | ReplayGain scale with preamp and clip guard | `jf-engine` `replaygain_scale` |
//! | Limiter at −1 dBFS, engaged only when gain can pass full scale or tracks overlap | `jf-engine` `limiter_needed` |
//! | Stream quality: lossless first | `jf-core` sources, `QUALITY` |

use std::ops::RangeInclusive;

use jf_plugin_model::Quality;
use serde::{Deserialize, Serialize};

/// Previous restarts the current track once this much of it has played.
pub const RESTART_THRESHOLD_MS: u64 = 3_000;
/// A play stopped early counts once this much has played (or half of a
/// shorter track).
pub const PLAY_COUNTS_AFTER_MS: u64 = 4 * 60 * 1_000;
/// Shorter overlaps join gaplessly instead.
pub const MIN_OVERLAP_MS: u64 = 50;
/// The limiter's ceiling in dBFS.
pub const LIMITER_CEILING_DB: f32 = -1.0;
/// The limiter's look-ahead (attack) in seconds.
pub const LIMITER_ATTACK_S: f32 = 0.0015;
/// The limiter's release time constant in seconds.
pub const LIMITER_RELEASE_S: f32 = 0.08;

/// Whether Previous restarts the current track instead of going back.
pub fn previous_restarts(position_ms: u64, active: bool) -> bool {
    active && position_ms > RESTART_THRESHOLD_MS
}

/// Whether a play counts: it reached its end (`natural`), or was left after
/// half of the track (at most four minutes). Earlier, it is a skip.
pub fn play_counts(played_ms: u64, duration_ms: Option<u64>, natural: bool) -> bool {
    natural || played_ms >= duration_ms.map_or(PLAY_COUNTS_AFTER_MS, |d| (d / 2).min(PLAY_COUNTS_AFTER_MS))
}

/// The key that joins tracks of one album gaplessly: `scope` (a source, or
/// a folder) and the album's names. No album tag, no key.
pub fn album_key(scope: Option<&str>, album: Option<&str>, artist: Option<&str>) -> Option<String> {
    let album = album.map(str::trim).filter(|a| !a.is_empty())?;
    Some(format!(
        "{}\u{1f}{}\u{1f}{}",
        scope.unwrap_or_default(),
        album.to_lowercase(),
        artist.unwrap_or_default().trim().to_lowercase()
    ))
}

/// How automatic transitions sound. Manual skips never crossfade. Missing
/// fields load as their defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct CrossfadeSettings {
    /// Crossfade automatic transitions; off joins every track gaplessly.
    pub enabled: bool,
    /// Overlap, 1000 to 12000 ms (clamped).
    pub duration_ms: u32,
    /// Two tracks of one album join gaplessly instead.
    pub album_continuity: bool,
}

impl CrossfadeSettings {
    /// Shortest and longest overlap, in milliseconds.
    pub const DURATION_RANGE_MS: RangeInclusive<u32> = 1_000..=12_000;

    /// The same settings with the duration in range.
    pub fn clamped(self) -> Self {
        CrossfadeSettings {
            duration_ms: self
                .duration_ms
                .clamp(*Self::DURATION_RANGE_MS.start(), *Self::DURATION_RANGE_MS.end()),
            ..self
        }
    }
}

impl Default for CrossfadeSettings {
    fn default() -> Self {
        CrossfadeSettings {
            enabled: false,
            duration_ms: 6_000,
            album_continuity: true,
        }
    }
}

/// How one track hands over to the next on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Transition {
    /// The next track starts the moment this one ends.
    Gapless,
    /// The two overlap for `overlap_ms`, faded with equal power: the
    /// outgoing side follows `cos`, the incoming side `sin` of
    /// `π/2 · progress`.
    Crossfade {
        /// Length of the overlap.
        overlap_ms: u64,
    },
}

/// The transition from the current track to the next. Durations may be
/// unknown: without the current one there is no crossfade (the engine plans
/// it from the track's length), without the next one only the current
/// track bounds the overlap.
pub fn transition(
    settings: CrossfadeSettings,
    current_album: Option<&str>,
    next_album: Option<&str>,
    current_ms: Option<u64>,
    next_ms: Option<u64>,
) -> Transition {
    if !settings.enabled {
        return Transition::Gapless;
    }
    if settings.album_continuity && matches!((current_album, next_album), (Some(a), Some(b)) if a == b) {
        return Transition::Gapless;
    }
    let Some(current_ms) = current_ms else {
        return Transition::Gapless;
    };
    let mut overlap = u64::from(settings.clamped().duration_ms).min(current_ms / 2);
    if let Some(next_ms) = next_ms {
        overlap = overlap.min(next_ms / 2);
    }
    if overlap < MIN_OVERLAP_MS {
        Transition::Gapless
    } else {
        Transition::Crossfade { overlap_ms: overlap }
    }
}

/// The equal-power fade curves for a crossfade, `points` samples each
/// (`[outgoing, incoming]`), for `AudioParam.setValueCurveAtTime`.
pub fn fade_curves(points: usize) -> [Vec<f32>; 2] {
    let points = points.max(2);
    let phase = |i: usize| std::f64::consts::FRAC_PI_2 * i as f64 / (points - 1) as f64;
    [
        (0..points).map(|i| phase(i).cos() as f32).collect(),
        (0..points).map(|i| phase(i).sin() as f32).collect(),
    ]
}

// ---- equaliser -------------------------------------------------------------------------------

/// Most bands an [`EqSettings`] may hold; extra bands are ignored.
pub const MAX_BANDS: usize = 10;
/// Band centres of the presets: the ISO octave frequencies of a 10-band EQ.
pub const PRESET_FREQS: [f32; MAX_BANDS] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];
/// One octave wide.
pub const PRESET_Q: f32 = 1.41;

/// Filter shape of an EQ band (`BiquadFilterNode.type`: `peaking`,
/// `lowshelf`, `highshelf`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BandKind {
    Peaking,
    LowShelf,
    HighShelf,
}

/// One EQ band.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EqBand {
    pub kind: BandKind,
    pub freq_hz: f32,
    pub gain_db: f32,
    pub q: f32,
    pub enabled: bool,
}

/// Equaliser settings: a preamp and up to [`MAX_BANDS`] bands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EqSettings {
    pub enabled: bool,
    pub preamp_db: f32,
    pub bands: Vec<EqBand>,
}

impl Default for EqSettings {
    /// The flat preset, switched off.
    fn default() -> Self {
        EqSettings {
            enabled: false,
            ..Self::flat()
        }
    }
}

impl EqSettings {
    /// Ten one-octave peaking bands at 0 dB.
    pub fn flat() -> Self {
        Self::graphic([0.0; MAX_BANDS])
    }

    /// Lifts the lowest octaves.
    pub fn bass_boost() -> Self {
        Self::graphic([6.0, 5.0, 4.0, 2.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0])
    }

    /// Brings voices forward.
    pub fn vocal() -> Self {
        Self::graphic([-2.0, -2.0, -1.0, 0.0, 1.5, 3.0, 3.0, 2.0, 0.0, -1.0])
    }

    /// Lifts the highest octaves.
    pub fn treble() -> Self {
        Self::graphic([0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.5, 3.0, 4.5, 6.0])
    }

    /// The presets, by name: `flat`, `bass_boost`, `vocal`, `treble`.
    pub fn presets() -> Vec<(&'static str, EqSettings)> {
        vec![
            ("flat", Self::flat()),
            ("bass_boost", Self::bass_boost()),
            ("vocal", Self::vocal()),
            ("treble", Self::treble()),
        ]
    }

    fn graphic(gains: [f32; MAX_BANDS]) -> Self {
        EqSettings {
            enabled: true,
            preamp_db: 0.0,
            bands: PRESET_FREQS
                .iter()
                .zip(gains)
                .map(|(&freq_hz, gain_db)| EqBand {
                    kind: BandKind::Peaking,
                    freq_hz,
                    gain_db,
                    q: PRESET_Q,
                    enabled: true,
                })
                .collect(),
        }
    }

    /// The bands in use: enabled, within the first [`MAX_BANDS`].
    pub fn active_bands(&self) -> impl Iterator<Item = &EqBand> {
        self.bands.iter().take(MAX_BANDS).filter(|b| b.enabled)
    }

    /// Whether these settings change the signal at all.
    pub fn is_active(&self) -> bool {
        self.enabled && (self.preamp_db != 0.0 || self.active_bands().any(|b| b.gain_db != 0.0))
    }

    /// An upper bound on the gain at any frequency, in dB.
    pub fn max_boost_db(&self) -> f32 {
        if !self.is_active() {
            return 0.0;
        }
        self.preamp_db
            + self
                .active_bands()
                .map(|b| b.gain_db.clamp(0.0, 24.0))
                .sum::<f32>()
    }
}

// ---- ReplayGain and the limiter --------------------------------------------------------------

/// ReplayGain 2.0 values from tags: gains in dB, peaks as linear sample
/// peaks (1.0 = full scale).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ReplayGainInfo {
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_gain_db: Option<f32>,
    pub album_peak: Option<f32>,
}

/// Which ReplayGain value to apply.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReplayGainMode {
    #[default]
    Off,
    Track,
    Album,
}

/// A gain in dB as a linear factor.
pub fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// The linear gain ReplayGain asks for: the selected gain plus `preamp_db`,
/// reduced so the stated peak stays at full scale (the clip guard). `Track`
/// falls back to the album values and `Album` to the track values; without
/// tags (or `Off`) it is 1.0, so the preamp only applies to tagged files.
pub fn replaygain_scale(info: Option<&ReplayGainInfo>, mode: ReplayGainMode, preamp_db: f32) -> f32 {
    let Some(info) = info else {
        return 1.0;
    };
    let track = (info.track_gain_db, info.track_peak);
    let album = (info.album_gain_db, info.album_peak);
    let (gain_db, peak) = match mode {
        ReplayGainMode::Off => return 1.0,
        ReplayGainMode::Track if track.0.is_some() => track,
        ReplayGainMode::Track => album,
        ReplayGainMode::Album if album.0.is_some() => album,
        ReplayGainMode::Album => track,
    };
    let Some(gain_db) = gain_db.filter(|g| g.is_finite()) else {
        return 1.0;
    };
    let preamp_db = if preamp_db.is_finite() { preamp_db } else { 0.0 };
    let mut scale = db_to_linear((gain_db + preamp_db).clamp(-60.0, 30.0));
    if let Some(peak) = peak.filter(|p| p.is_finite() && *p > 0.0)
        && scale * peak > 1.0
    {
        scale = 1.0 / peak;
    }
    scale
}

/// Whether the limiter is engaged: some playing track's ReplayGain scale is
/// above unity, the EQ can boost, or two tracks overlap.
pub fn limiter_needed(replaygain_scales: &[f32], eq: &EqSettings, crossfading: bool) -> bool {
    let gain = replaygain_scales.iter().copied().fold(1.0f32, f32::max);
    gain > 1.0 + 1e-6 || eq.max_boost_db() > 0.0 || crossfading
}

// ---- streams ---------------------------------------------------------------------------------

/// The quality asked for first: the original file (the core's choice).
pub const FIRST_QUALITY: Quality = Quality::Lossless;

/// The quality to ask for after the browser could not play `failed`: each
/// step down makes the server convert more files (to the plugin's
/// conversion format), so a file the browser cannot decode gets converted.
pub fn next_quality(failed: Quality) -> Option<Quality> {
    match failed {
        Quality::Lossless => Some(Quality::High),
        Quality::High => Some(Quality::Low),
        Quality::Normal => Some(Quality::Low),
        Quality::Low => None,
    }
}

/// The levels the bar's quality menu offers for what plays (ADR-0026 §2),
/// best first: none for a file (it plays as it is), every level for a
/// source's stream (`policy` is its content policy; a plugin cannot list
/// its own levels before API 1.3).
pub fn offered_qualities(policy: Option<&jf_plugin_model::ContentPolicy>) -> Vec<Quality> {
    match policy {
        None => Vec::new(),
        Some(_) => vec![Quality::Lossless, Quality::High, Quality::Normal, Quality::Low],
    }
}

/// A MIME type for `HTMLMediaElement.canPlayType` from a plugin's codec
/// hint, when the hint names one.
pub fn codec_mime(codec: &str) -> Option<&'static str> {
    Some(match codec.trim().to_ascii_lowercase().as_str() {
        "flac" => "audio/flac",
        "mp3" | "mpeg" => "audio/mpeg",
        "aac" | "m4a" | "mp4" => "audio/mp4; codecs=\"mp4a.40.2\"",
        "alac" => "audio/mp4; codecs=\"alac\"",
        "opus" => "audio/ogg; codecs=\"opus\"",
        "vorbis" | "ogg" | "oga" => "audio/ogg; codecs=\"vorbis\"",
        "wav" | "pcm" => "audio/wav",
        "aiff" | "aif" => "audio/aiff",
        "webm" | "webma" => "audio/webm",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previous_and_play_counts() {
        assert!(!previous_restarts(3_000, true));
        assert!(previous_restarts(3_001, true));
        assert!(!previous_restarts(10_000, false));
        assert!(play_counts(0, Some(200_000), true));
        assert!(!play_counts(99_999, Some(200_000), false));
        assert!(play_counts(100_000, Some(200_000), false));
        assert!(play_counts(240_000, Some(900_000), false), "at most four minutes");
        assert!(!play_counts(239_999, None, false));
    }

    #[test]
    fn album_keys_ignore_case_and_need_an_album() {
        assert_eq!(
            album_key(Some("p"), Some(" Blue "), Some("Artist")),
            album_key(Some("p"), Some("blue"), Some("artist "))
        );
        assert_ne!(
            album_key(Some("p"), Some("Blue"), None),
            album_key(Some("q"), Some("Blue"), None)
        );
        assert_eq!(album_key(Some("p"), Some("  "), None), None);
    }

    #[test]
    fn transitions_follow_the_engine() {
        let on = CrossfadeSettings {
            enabled: true,
            ..CrossfadeSettings::default()
        };
        let a = Some("album");
        assert_eq!(
            transition(CrossfadeSettings::default(), None, None, Some(1), Some(1)),
            Transition::Gapless
        );
        assert_eq!(
            transition(on, a, a, Some(200_000), Some(200_000)),
            Transition::Gapless
        );
        let off = CrossfadeSettings {
            album_continuity: false,
            ..on
        };
        assert_eq!(
            transition(off, a, a, Some(200_000), Some(200_000)),
            Transition::Crossfade { overlap_ms: 6_000 }
        );
        assert_eq!(
            transition(on, a, None, Some(8_000), Some(200_000)),
            Transition::Crossfade { overlap_ms: 4_000 },
            "never more than half of the outgoing track"
        );
        assert_eq!(
            transition(on, None, None, Some(200_000), Some(3_000)),
            Transition::Crossfade { overlap_ms: 1_500 },
            "never more than half of the incoming track"
        );
        assert_eq!(transition(on, None, None, Some(80), None), Transition::Gapless);
        assert_eq!(
            transition(on, None, None, None, Some(200_000)),
            Transition::Gapless
        );
        let long = CrossfadeSettings {
            duration_ms: 60_000,
            ..on
        };
        assert_eq!(
            transition(long, None, None, Some(600_000), Some(600_000)),
            Transition::Crossfade { overlap_ms: 12_000 }
        );
    }

    #[test]
    fn fades_keep_power() {
        let [out, inc] = fade_curves(33);
        assert_eq!((out[0], inc[0]), (1.0, 0.0));
        assert!((out[32]).abs() < 1e-6 && (inc[32] - 1.0).abs() < 1e-6);
        for (o, i) in out.iter().zip(&inc) {
            assert!((o * o + i * i - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn limiter_engages_only_when_needed() {
        let flat = EqSettings::flat();
        assert!(!limiter_needed(&[1.0, 0.5], &flat, false));
        assert!(limiter_needed(&[1.2], &flat, false));
        assert!(limiter_needed(&[1.0], &EqSettings::bass_boost(), false));
        assert!(limiter_needed(&[], &flat, true));
        assert!(!limiter_needed(&[], &EqSettings::default(), false));
    }

    #[test]
    fn files_offer_no_levels_and_streams_all() {
        assert!(offered_qualities(None).is_empty());
        let policy = jf_plugin_model::ContentPolicy::default();
        assert_eq!(
            offered_qualities(Some(&policy)),
            [Quality::Lossless, Quality::High, Quality::Normal, Quality::Low]
        );
    }

    #[test]
    fn quality_steps_down_and_codecs_map() {
        assert_eq!(next_quality(Quality::Lossless), Some(Quality::High));
        assert_eq!(next_quality(Quality::High), Some(Quality::Low));
        assert_eq!(next_quality(Quality::Low), None);
        assert_eq!(codec_mime("FLAC"), Some("audio/flac"));
        assert_eq!(codec_mime("weird"), None);
    }
}
