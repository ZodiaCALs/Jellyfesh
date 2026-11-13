//! Changing the output while playing: the position carries over to the new output (at its own
//! sample rate), a failed switch keeps the old output, and device notifications are harmless
//! when the output is not a device.

mod common;

use std::f64::consts::PI;
use std::time::Duration;

use common::*;
use jf_engine::{Engine, EngineConfig, EngineEvent, OutputKind};

/// Phase of a 1 kHz sine in `x` (starting at `first`) against `sin(2π·1000·n/rate)`, as a time
/// offset in seconds: where the sine "is" relative to the reference.
fn sine_offset_s(x: &[f32], first: usize, rate: u32) -> f64 {
    let w = 2.0 * PI * 1_000.0 / f64::from(rate);
    let (mut ss, mut cc, mut sc, mut xs, mut xc) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (i, &v) in x.iter().enumerate() {
        let n = (first + i) as f64;
        let (s, c) = ((w * n).sin(), (w * n).cos());
        ss += s * s;
        cc += c * c;
        sc += s * c;
        xs += f64::from(v) * s;
        xc += f64::from(v) * c;
    }
    let det = ss * cc - sc * sc;
    let a = (xs * cc - xc * sc) / det;
    let b = (xc * ss - xs * sc) / det;
    b.atan2(a) / w / f64::from(rate)
}

#[test]
fn switching_outputs_continues_from_the_audible_position() {
    let dir = tempfile::tempdir().unwrap();
    let tone = dir.path().join("tone.wav");
    // 4 s of a 1 kHz sine at 44.1 kHz.
    sine_wav(&tone, 1_000.0, 44_100, 4 * 44_100, 0.5);
    let (first, second) = (dir.path().join("first.wav"), dir.path().join("second.wav"));
    let config = EngineConfig {
        output: OutputKind::WavFile {
            path: first.clone(),
            realtime: true,
        },
        buffer_ms: 200,
        ..EngineConfig::default()
    };
    let engine = Engine::new(config).unwrap();
    let events = engine.events();
    engine.set_queue(vec![item(1, &tone)], 0);
    engine.play();
    std::thread::sleep(Duration::from_millis(900));
    // The new output is a fresh WAV file at its default 48 kHz: playback is resampled.
    engine.set_output(OutputKind::WavFile {
        path: second.clone(),
        realtime: true,
    });
    assert!(engine.wait_until_idle(IDLE_TIMEOUT));
    drop(engine);
    let events: Vec<EngineEvent> = events.try_iter().collect();

    let (rate1, a) = read_wav(&first);
    let (rate2, b) = read_wav(&second);
    let a: Vec<f32> = a.iter().step_by(2).copied().collect();
    let b: Vec<f32> = b.iter().step_by(2).copied().collect();
    assert_eq!((rate1, rate2), (44_100, 48_000));
    // The first file holds what was heard there, its last 10 ms fading out; the second
    // continues from that point in the track (after its own 10 ms fade-in).
    let heard_s = a.len() as f64 / 44_100.0;
    let total_s = heard_s + b.len() as f64 / 48_000.0;
    println!(
        "switch after {heard_s:.4} s; {:.4} s on the new output; {total_s:.5} s in all",
        b.len() as f64 / 48_000.0
    );
    assert!(
        (total_s - 4.0).abs() < 0.002,
        "nothing lost or repeated: {total_s}"
    );
    // Where the sine in the second file is, against the track's own timeline.
    let skip = 48_000 / 50; // past the fade-in
    let offset = sine_offset_s(&b[skip..skip + 4_800], skip, 48_000);
    // The track continues at `heard_s`: its phase there, minus the new file's own phase.
    let period = 1.0 / 1_000.0;
    let expected = (heard_s % period + period) % period;
    let found = (offset % period + period) % period;
    let error = ((found - expected + period / 2.0).rem_euclid(period)) - period / 2.0;
    println!("phase continuity across the switch: {:+.1} us", error * 1e6);
    assert!(error.abs() < 50e-6, "the track jumped by {:.1} us", error * 1e6);
    // No click on either side of the switch. The track's own abrupt end rings through the
    // resampler, so the last frames of the second file are left out (as in the gapless tests).
    let bound = sine_step_bound(1_000.0, 44_100, 0.5) * 1.01;
    let (step_a, _) = max_step(&a);
    let (step_b, _) = max_step(&b[..b.len() - 200]);
    println!("largest steps: first {step_a:.4}, second {step_b:.4} (bound {bound:.4})");
    assert!(step_a <= bound && step_b <= bound, "{step_a} {step_b}");
    assert!(
        events.iter().all(|e| !matches!(e, EngineEvent::Error { .. })),
        "{events:?}"
    );
    assert!(events.iter().any(|e| matches!(e, EngineEvent::QueueEnded)));
}

#[test]
fn a_failed_switch_keeps_the_old_output() {
    let dir = tempfile::tempdir().unwrap();
    let tone = dir.path().join("tone.wav");
    let input = stereo(&sine(440.0, 44_100, 0, 44_100, 0.5));
    write_wav_f32(&tone, 44_100, 2, &input);
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
    let events = engine.events();
    engine.set_queue(vec![item(1, &tone)], 0);
    engine.play();
    std::thread::sleep(Duration::from_millis(300));
    engine.set_output(OutputKind::WavFile {
        path: dir.path().join("missing-dir").join("x.wav"),
        realtime: true,
    });
    engine.devices_changed();
    assert!(engine.wait_until_idle(IDLE_TIMEOUT));
    drop(engine);

    let errors: Vec<_> = events
        .try_iter()
        .filter_map(|e| match e {
            EngineEvent::Error { id, message } => Some((id, message)),
            _ => None,
        })
        .collect();
    println!("errors: {errors:?}");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].0, None, "an output problem, not the item's");
    let (_, samples) = read_wav(&out);
    assert_eq!(
        samples.len(),
        input.len(),
        "playback went on, complete, on the old output"
    );
    assert!(samples.iter().zip(&input).all(|(&a, &b)| a == b as f32));
}

#[test]
fn switching_while_stopped_takes_effect_on_play() {
    let dir = tempfile::tempdir().unwrap();
    let tone = dir.path().join("tone.wav");
    sine_wav(&tone, 440.0, 44_100, 22_050, 0.5);
    let engine = Engine::new(EngineConfig {
        output: OutputKind::Null { realtime: false },
        ..EngineConfig::default()
    })
    .unwrap();
    let out = dir.path().join("out.wav");
    engine.set_output(OutputKind::WavFile {
        path: out.clone(),
        realtime: false,
    });
    engine.set_queue(vec![item(1, &tone)], 0);
    engine.play();
    assert!(engine.wait_until_idle(IDLE_TIMEOUT));
    drop(engine);
    let (rate, samples) = read_wav(&out);
    assert_eq!(
        rate, 44_100,
        "a fresh file output still takes the first track's rate"
    );
    assert_eq!(samples.len(), 2 * 22_050);
}
