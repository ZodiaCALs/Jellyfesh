//! The rules `jf-web` mirrors from `jf-engine` and `jf-core` (which do not
//! build for the browser) must stay identical to the originals: same values,
//! same JSON. Runs on the host only.

#![cfg(not(target_arch = "wasm32"))]

use jf_web::rules::{self, CrossfadeSettings, EqSettings, ReplayGainInfo, ReplayGainMode};
use jf_web::settings::WebSettings;
use serde_json::Value;

fn as_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap()
}

#[test]
fn equaliser_presets_match_the_engine() {
    let pairs = [
        (EqSettings::flat(), jf_engine::EqSettings::flat()),
        (EqSettings::bass_boost(), jf_engine::EqSettings::bass_boost()),
        (EqSettings::vocal(), jf_engine::EqSettings::vocal()),
        (EqSettings::treble(), jf_engine::EqSettings::treble()),
        (EqSettings::default(), jf_engine::EqSettings::default()),
    ];
    for (ours, engine) in pairs {
        assert_eq!(as_json(&ours), as_json(&engine));
        assert_eq!(ours.max_boost_db(), engine.max_boost_db());
        assert_eq!(ours.is_active(), engine.is_active());
    }
    assert_eq!(rules::MAX_BANDS, jf_engine::dsp::MAX_BANDS);
}

#[test]
fn crossfade_settings_match_the_engine() {
    assert_eq!(
        as_json(&CrossfadeSettings::default()),
        as_json(&jf_engine::CrossfadeSettings::default())
    );
    assert_eq!(
        CrossfadeSettings::DURATION_RANGE_MS,
        jf_engine::CrossfadeSettings::DURATION_RANGE_MS
    );
}

#[test]
fn replaygain_matches_the_engine() {
    let gains = [None, Some(-60.0), Some(-6.2), Some(0.0), Some(3.5), Some(40.0)];
    let peaks = [None, Some(0.1), Some(0.5), Some(0.99), Some(2.0)];
    let modes = [
        (ReplayGainMode::Off, jf_engine::ReplayGainMode::Off),
        (ReplayGainMode::Track, jf_engine::ReplayGainMode::Track),
        (ReplayGainMode::Album, jf_engine::ReplayGainMode::Album),
    ];
    for track_gain_db in gains {
        for album_gain_db in [None, Some(-3.0)] {
            for track_peak in peaks {
                for preamp in [0.0, 6.0, -15.0, f32::NAN] {
                    let ours = ReplayGainInfo {
                        track_gain_db,
                        track_peak,
                        album_gain_db,
                        album_peak: Some(0.9),
                    };
                    let engine = jf_engine::ReplayGainInfo {
                        track_gain_db,
                        track_peak,
                        album_gain_db,
                        album_peak: Some(0.9),
                    };
                    assert_eq!(as_json(&ours), as_json(&engine));
                    for (mode, engine_mode) in modes {
                        let a = rules::replaygain_scale(Some(&ours), mode, preamp);
                        let b = jf_engine::dsp::replaygain_scale(Some(&engine), engine_mode, preamp);
                        assert_eq!(a.to_bits(), b.to_bits(), "{ours:?} {mode:?} {preamp}");
                    }
                }
            }
        }
    }
    assert_eq!(rules::replaygain_scale(None, ReplayGainMode::Track, 6.0), 1.0);
    assert_eq!(
        as_json(&ReplayGainMode::Album),
        as_json(&jf_engine::ReplayGainMode::Album)
    );
    for db in [-60.0, -6.0, 0.0, 1.5, 24.0] {
        assert_eq!(rules::db_to_linear(db), jf_engine::dsp::db_to_linear(db));
    }
}

#[test]
fn settings_share_the_cores_names_and_defaults() {
    let ours = as_json(&WebSettings::default());
    let core = as_json(&jf_core::Settings::default());
    let shared = [
        "appearance",
        "style",
        "hue_source",
        "reduce_effects",
        "transparency",
        "lyrics_size",
        "lyrics_align",
        "show_translation",
        "show_romanization",
        "crossfade",
        "replaygain_mode",
        "preamp_db",
        "eq",
        "resume_on_launch",
    ];
    for key in shared {
        assert_eq!(ours[key], core[key], "{key}");
    }
    // The core's settings load into ours (and keep their values).
    let core_settings = jf_core::Settings {
        style: jf_core::BackdropStyle::Misty,
        transparency: 0.2,
        lyrics_size: jf_core::LyricsSize::Large,
        lyrics_align: jf_core::LyricsAlign::Center,
        crossfade: jf_core::CrossfadeSettings {
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let loaded: WebSettings = serde_json::from_value(as_json(&core_settings)).unwrap();
    assert_eq!(as_json(&loaded)["style"], "misty");
    assert_eq!(as_json(&loaded)["lyrics_size"], "large");
    assert_eq!(as_json(&loaded)["lyrics_align"], "center");
    assert!((loaded.transparency - 0.2).abs() < 1e-6);
    assert!(loaded.crossfade.enabled);
}
