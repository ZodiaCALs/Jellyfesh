//! ReplayGain: the scale factor for a track.

use super::db_to_linear;
use crate::types::{ReplayGainInfo, ReplayGainMode};

/// The linear gain ReplayGain asks for: the selected gain plus `preamp_db`, reduced if needed
/// so that the stated peak does not go past full scale (the clip guard).
///
/// `Track` uses the track gain and peak, falling back to the album values; `Album` the other
/// way round. Without tags (or with `Off`) the result is 1.0: the preamp only applies to
/// tagged files, so untagged ones are not made louder or quieter than they are.
///
/// `info` may be a `&ReplayGainInfo` or an `Option<&ReplayGainInfo>` (such as
/// `item.replaygain.as_ref()`).
pub fn replaygain_scale<'a>(
    info: impl Into<Option<&'a ReplayGainInfo>>,
    mode: ReplayGainMode,
    preamp_db: f32,
) -> f32 {
    let Some(info) = info.into() else {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn db(scale: f32) -> f32 {
        20.0 * scale.log10()
    }

    const INFO: ReplayGainInfo = ReplayGainInfo {
        track_gain_db: Some(-6.0),
        track_peak: Some(0.5),
        album_gain_db: Some(-3.0),
        album_peak: Some(0.9),
    };

    #[test]
    fn off_and_untagged_are_unity() {
        assert_eq!(replaygain_scale(&INFO, ReplayGainMode::Off, 6.0), 1.0);
        assert_eq!(replaygain_scale(None, ReplayGainMode::Track, 6.0), 1.0);
        assert_eq!(
            replaygain_scale(Some(&ReplayGainInfo::default()), ReplayGainMode::Album, 6.0),
            1.0
        );
    }

    #[test]
    fn modes_select_their_gain() {
        assert!((db(replaygain_scale(Some(&INFO), ReplayGainMode::Track, 0.0)) + 6.0).abs() < 1e-4);
        assert!((db(replaygain_scale(Some(&INFO), ReplayGainMode::Album, 0.0)) + 3.0).abs() < 1e-4);
        // The preamp adds on top.
        assert!((db(replaygain_scale(Some(&INFO), ReplayGainMode::Album, 2.0)) + 1.0).abs() < 1e-4);
    }

    #[test]
    fn modes_fall_back_to_the_other_gain() {
        let track_only = ReplayGainInfo {
            track_gain_db: Some(-4.0),
            ..Default::default()
        };
        let album_only = ReplayGainInfo {
            album_gain_db: Some(-2.0),
            ..Default::default()
        };
        assert!((db(replaygain_scale(Some(&track_only), ReplayGainMode::Album, 0.0)) + 4.0).abs() < 1e-4);
        assert!((db(replaygain_scale(Some(&album_only), ReplayGainMode::Track, 0.0)) + 2.0).abs() < 1e-4);
    }

    #[test]
    fn clip_guard_uses_the_selected_peak() {
        // Album: -3 dB + 6 dB preamp = +3 dB (x1.41), but the album peak 0.9 allows only x1.11.
        let album = replaygain_scale(Some(&INFO), ReplayGainMode::Album, 6.0);
        assert!((album - 1.0 / 0.9).abs() < 1e-6, "{album}");
        // Track: -6 dB + 12 dB = +6 dB (x1.995); peak 0.5 allows x2.
        let track = replaygain_scale(Some(&INFO), ReplayGainMode::Track, 12.0);
        assert!((db(track) - 6.0).abs() < 1e-3, "{track}");
        // +14 dB would push the 0.5 peak over full scale: capped at x2.
        let capped = replaygain_scale(Some(&INFO), ReplayGainMode::Track, 20.0);
        assert!((capped - 2.0).abs() < 1e-6, "{capped}");
    }
}
