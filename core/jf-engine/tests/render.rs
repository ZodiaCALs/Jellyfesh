//! Offline renders through the whole engine, analysed for frequency, level and timing.

mod common;

use common::*;
use jf_engine::{EqBand, EqSettings, PlayItem, ReplayGainInfo, ReplayGainMode};

#[test]
fn wav_at_the_output_rate_renders_bit_exact() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let input = stereo(&sine(1_000.0, 44_100, 0, 44_100, 0.5));
    write_wav_f32(&path, 44_100, 2, &input);

    let render = render(dir.path(), vec![item(1, &path)], None, |_| {});

    assert_eq!(render.rate, 44_100, "the WAV sink takes the first track's rate");
    assert_eq!(render.frames(), 44_100);
    let exact = render.samples.iter().zip(&input).all(|(a, &b)| *a == b as f32);
    assert!(exact, "unity gain at the source rate must not change a sample");
    let freq = zero_crossing_frequency(&render.left(), render.rate);
    println!("WAV 1 kHz -> {freq:.3} Hz");
    assert!((freq - 1_000.0).abs() < 0.1, "{freq}");
}

#[test]
fn flac_keeps_its_frequency() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.flac");
    let expected = sine_flac(&path, 997.0, 48_000, 0, 96_000, 0.5);

    let render = render(dir.path(), vec![item(1, &path)], None, |_| {});

    assert_eq!(render.rate, 48_000);
    assert_eq!(render.left(), expected, "FLAC decodes losslessly");
    let freq = zero_crossing_frequency(&render.left(), render.rate);
    println!("FLAC 997 Hz -> {freq:.3} Hz");
    assert!((freq - 997.0).abs() < 0.1, "{freq}");
}

#[test]
fn resampling_48k_to_44k1_keeps_tone_and_duration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone48.flac");
    sine_flac(&path, 1_000.0, 48_000, 0, 48_000 * 2, 0.5);

    let render = render(dir.path(), vec![item(1, &path)], Some(44_100), |_| {});

    assert_eq!(render.rate, 44_100);
    let seconds = render.frames() as f64 / 44_100.0;
    let left = render.left();
    let freq = zero_crossing_frequency(&left[2_000..left.len() - 2_000], 44_100);
    let level = tone_amplitude(&left[4_410..4_410 * 9], 1_000.0, 44_100);
    println!("48 kHz -> 44.1 kHz: {seconds:.5} s, {freq:.3} Hz, amplitude {level:.4}");
    assert!((seconds - 2.0).abs() < 0.02, "duration {seconds}");
    assert_eq!(render.frames(), 88_200, "n input frames become round(n x ratio)");
    assert!((freq - 1_000.0).abs() < 10.0, "{freq}");
    assert!((freq - 1_000.0).abs() < 0.1, "tighter than required: {freq}");
    assert!((db(level / 0.5)).abs() < 0.1, "level {level}");
}

/// Level of a rendered 1 kHz tone (steady state) relative to `reference` amplitude.
fn rendered_gain_db(render: &Render, freq: f64, reference: f64) -> f64 {
    let left = render.left();
    let rate = render.rate;
    let period_frames = f64::from(rate) / freq;
    // Whole periods from the middle of the render.
    let periods = ((left.len() / 2) as f64 / period_frames).floor();
    let len = (periods * period_frames).round() as usize;
    let start = left.len() / 4;
    db(tone_amplitude(&left[start..start + len], freq, rate) / reference)
}

#[test]
fn replaygain_minus_6_db_halves_the_amplitude() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    sine_wav(&path, 1_000.0, 44_100, 44_100, 0.5);
    let tagged = PlayItem {
        replaygain: Some(ReplayGainInfo {
            track_gain_db: Some(-6.0206),
            ..Default::default()
        }),
        ..item(1, &path)
    };

    let render = render(dir.path(), vec![tagged], None, |e| {
        e.set_replaygain(ReplayGainMode::Track, 0.0)
    });

    let gain = rendered_gain_db(&render, 1_000.0, 0.5);
    let ratio = f64::from(peak(&render.samples)) / 0.5;
    println!("ReplayGain -6.02 dB: measured {gain:.3} dB, peak ratio {ratio:.4}");
    assert!((gain + 6.0).abs() < 0.2, "{gain}");
    assert!((ratio - 0.5).abs() < 0.01, "{ratio}");
}

#[test]
fn replaygain_modes_and_the_peak_clip_guard() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    sine_wav(&path, 1_000.0, 44_100, 44_100, 0.5);
    // Track: +3 dB, but the tagged 0.9 peak only allows x1.111 (+0.915 dB).
    // Album: -6 dB with a 0.5 peak, so no guard.
    let info = ReplayGainInfo {
        track_gain_db: Some(3.0),
        track_peak: Some(0.9),
        album_gain_db: Some(-6.0),
        album_peak: Some(0.5),
    };
    let tagged = PlayItem {
        replaygain: Some(info),
        ..item(1, &path)
    };

    let mut results = Vec::new();
    for (mode, expected) in [
        (ReplayGainMode::Off, 0.0),
        (ReplayGainMode::Album, -6.0),
        (ReplayGainMode::Track, 20.0 * (1.0f64 / 0.9).log10()),
    ] {
        let sub = dir.path().join(format!("{mode:?}"));
        std::fs::create_dir(&sub).unwrap();
        let render = render(&sub, vec![tagged.clone()], None, |e| e.set_replaygain(mode, 0.0));
        let gain = rendered_gain_db(&render, 1_000.0, 0.5);
        results.push(format!("{mode:?} {gain:+.3} dB (expected {expected:+.3})"));
        assert!(
            (gain - expected).abs() < 0.2,
            "{mode:?}: {gain} dB, expected {expected}"
        );
    }
    println!("ReplayGain modes: {}", results.join(", "));
}

#[test]
fn eq_peaking_band_boosts_1khz_and_leaves_other_frequencies() {
    let dir = tempfile::tempdir().unwrap();
    let eq = EqSettings {
        enabled: true,
        preamp_db: 0.0,
        bands: vec![EqBand::peaking(1_000.0, 6.0, 1.0)],
    };
    let mut results = Vec::new();
    for (freq, expected) in [(1_000.0, 6.0), (100.0, 0.0), (10_000.0, 0.0)] {
        let sub = dir.path().join(format!("{freq}"));
        std::fs::create_dir(&sub).unwrap();
        let path = sub.join("tone.wav");
        sine_wav(&path, freq, 44_100, 44_100, 0.25);
        let eq = eq.clone();
        let render = render(&sub, vec![item(1, &path)], None, move |e| e.set_eq(eq));
        let gain = rendered_gain_db(&render, freq, 0.25);
        results.push(format!("{freq} Hz {gain:+.3} dB"));
        assert!(
            (gain - expected).abs() < 0.5,
            "{freq} Hz: {gain} dB, expected {expected}"
        );
    }
    println!("EQ +6 dB at 1 kHz (Q 1): {}", results.join(", "));
}

#[test]
fn limiter_holds_the_ceiling_with_a_12_db_preamp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    sine_wav(&path, 1_000.0, 44_100, 44_100, 0.5);
    // 0 dB track gain with no peak tag: +12 dB preamp drives the 0.5 sine to about 2.0.
    let tagged = PlayItem {
        replaygain: Some(ReplayGainInfo {
            track_gain_db: Some(0.0),
            ..Default::default()
        }),
        ..item(1, &path)
    };

    let render = render(dir.path(), vec![tagged], None, |e| {
        e.set_replaygain(ReplayGainMode::Track, 12.0)
    });

    let ceiling = 10f32.powf(-1.0 / 20.0);
    let level = peak(&render.samples);
    println!("limiter: +12 dB preamp on a 0.5 sine -> peak {level:.5} (ceiling {ceiling:.5})");
    assert!(level <= ceiling, "{level} > {ceiling}");
    assert!(level > ceiling * 0.97, "limited too hard: {level}");
    assert_eq!(render.frames(), 44_100, "the limiter keeps the stream length");
}

#[test]
fn limiter_is_transparent_below_its_ceiling() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    sine_wav(&path, 1_000.0, 44_100, 44_100, 0.25);
    // +6 dB engages the limiter, but 0.5 stays under the -1 dBFS ceiling.
    let tagged = PlayItem {
        replaygain: Some(ReplayGainInfo {
            track_gain_db: Some(6.0206),
            ..Default::default()
        }),
        ..item(1, &path)
    };

    let render = render(dir.path(), vec![tagged], None, |e| {
        e.set_replaygain(ReplayGainMode::Track, 0.0)
    });

    let expected = stereo(&sine(1_000.0, 44_100, 0, 44_100, 0.25));
    let worst = render
        .samples
        .iter()
        .zip(&expected)
        .fold(0.0f64, |m, (&a, &b)| m.max((f64::from(a) - 2.0 * b).abs()));
    println!("limiter below ceiling: worst deviation {worst:.2e}");
    assert_eq!(render.frames(), 44_100);
    assert!(worst < 1e-6, "{worst}");
}

#[test]
fn volume_scales_the_output() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    sine_wav(&path, 1_000.0, 44_100, 44_100, 0.5);

    let render = render(dir.path(), vec![item(1, &path)], None, |e| e.set_volume(0.5));

    let gain = rendered_gain_db(&render, 1_000.0, 0.5);
    assert!((gain + 6.0206).abs() < 0.05, "{gain}");
}

#[test]
fn realtime_wav_output_is_paced_and_complete() {
    use jf_engine::{Engine, EngineConfig, OutputKind};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let input = stereo(&sine(1_000.0, 44_100, 0, 22_050, 0.5));
    write_wav_f32(&path, 44_100, 2, &input);
    let out = dir.path().join("out.wav");
    let config = EngineConfig {
        output: OutputKind::WavFile {
            path: out.clone(),
            realtime: true,
        },
        buffer_ms: 100,
        ..EngineConfig::default()
    };

    let engine = Engine::new(config).unwrap();
    let start = std::time::Instant::now();
    engine.set_queue(vec![item(1, &path)], 0);
    engine.play();
    assert!(engine.wait_until_idle(IDLE_TIMEOUT));
    let elapsed = start.elapsed();
    drop(engine);

    let (rate, samples) = read_wav(&out);
    println!("real-time WAV: 0.5 s of audio took {elapsed:?}");
    assert_eq!(rate, 44_100);
    assert!(elapsed >= std::time::Duration::from_millis(450), "{elapsed:?}");
    assert_eq!(samples.len(), input.len());
    assert!(samples.iter().zip(&input).all(|(&a, &b)| a == b as f32));
}
