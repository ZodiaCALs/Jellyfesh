//! Crossfades: equal-power curves measured sample by sample, exact overlaps, album continuity,
//! short tracks, track events and anchors during an overlap, and manual skips that neither
//! crossfade nor click.
//!
//! The curve tests play constant (DC) levels: the outgoing track on the left channel only, the
//! incoming one on the right only, so each channel of the render is one track's gain times its
//! level, sample by sample.

mod common;

use std::f64::consts::FRAC_PI_2;
use std::path::Path;
use std::time::Duration;

use common::*;
use jf_engine::{CrossfadeSettings, Engine, EngineConfig, EngineEvent, OutputKind, PlayItem};

const RATE: u32 = 44_100;
const LEVEL: f64 = 0.5;

fn crossfade(duration_ms: u32) -> CrossfadeSettings {
    CrossfadeSettings {
        enabled: true,
        duration_ms,
        album_continuity: true,
    }
}

/// `seconds` of a constant level on the left (`left: true`) or right channel.
fn dc_wav(path: &Path, seconds: f64, left: bool) {
    let frames = (seconds * f64::from(RATE)).round() as usize;
    let frame = if left { [LEVEL, 0.0] } else { [0.0, LEVEL] };
    let samples: Vec<f64> = (0..frames).flat_map(|_| frame).collect();
    write_wav_f32(path, RATE, 2, &samples);
}

fn track_events(events: &[EngineEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::TrackStarted { index, id, .. } => Some(format!("start {index}:{id}")),
            EngineEvent::TrackEnded { index, id } => Some(format!("end {index}:{id}")),
            EngineEvent::QueueEnded => Some("queue end".into()),
            _ => None,
        })
        .collect()
}

/// Gains of the two channels at frame `n`, relative to [`LEVEL`].
fn gains(samples: &[f32], n: usize) -> (f64, f64) {
    (
        f64::from(samples[2 * n]) / LEVEL,
        f64::from(samples[2 * n + 1]) / LEVEL,
    )
}

/// Checks an overlap over `[start, start + len)` where the left channel fades out and the right
/// one in; returns the largest deviation of the summed power from unity, in dB.
fn check_overlap(label: &str, samples: &[f32], start: usize, len: usize, out_left: bool) -> f64 {
    let mut worst_curve: f64 = 0.0;
    let mut worst_power_db: f64 = 0.0;
    for n in start..start + len {
        let (left, right) = gains(samples, n);
        let (outgoing, incoming) = if out_left { (left, right) } else { (right, left) };
        let phase = (n - start) as f64 / len as f64;
        worst_curve = worst_curve
            .max((outgoing - (FRAC_PI_2 * phase).cos()).abs())
            .max((incoming - (FRAC_PI_2 * phase).sin()).abs());
        let power_db = 10.0 * (outgoing * outgoing + incoming * incoming).log10();
        worst_power_db = worst_power_db.max(power_db.abs());
    }
    println!(
        "{label}: {len} frames from {start}; curves within {worst_curve:.2e} of cos/sin, \
         summed power within {worst_power_db:.5} dB of unity"
    );
    assert!(worst_curve < 1e-4, "{label}: curve off by {worst_curve}");
    assert!(worst_power_db < 0.2, "{label}: power off by {worst_power_db} dB");
    worst_power_db
}

#[test]
fn equal_power_curves_with_an_exact_overlap() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.wav"), dir.path().join("b.wav"));
    dc_wav(&a, 3.0, true);
    dc_wav(&b, 2.5, false);

    let render = render(dir.path(), vec![item(1, &a), item(2, &b)], None, |e| {
        e.set_crossfade(crossfade(1_000))
    });

    // 3 s + 2.5 s with 1 s overlapped.
    let overlap = RATE as usize;
    let fade_start = 2 * RATE as usize;
    assert_eq!(render.frames(), (5.5 * f64::from(RATE)) as usize - overlap);
    let samples = &render.samples;
    // Outside the overlap each track plays alone at its level.
    for n in (0..fade_start).step_by(997) {
        assert_eq!(gains(samples, n), (1.0, 0.0), "frame {n}");
    }
    for n in (fade_start + overlap..render.frames()).step_by(997) {
        assert_eq!(gains(samples, n), (0.0, 1.0), "frame {n}");
    }
    check_overlap("crossfade 1 s", samples, fade_start, overlap, true);
    // The overlap is exactly as long as asked: the incoming track is silent before it (its
    // gain is sin 0 at the first frame) and the outgoing one is gone after it.
    let first_in = (0..render.frames()).find(|&n| samples[2 * n + 1] != 0.0).unwrap();
    let last_out = (0..render.frames())
        .rev()
        .find(|&n| samples[2 * n] != 0.0)
        .unwrap();
    println!("incoming from frame {first_in}, outgoing until frame {last_out}");
    assert_eq!(first_in, fade_start + 1);
    assert_eq!(last_out, fade_start + overlap - 1);
    assert_eq!(
        track_events(&render.events),
        ["start 0:1", "start 1:2", "end 0:1", "end 1:2", "queue end"]
    );
}

#[test]
fn the_anchor_follows_the_incoming_track() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.wav"), dir.path().join("b.wav"));
    dc_wav(&a, 3.0, true);
    dc_wav(&b, 2.5, false);

    let render = render(dir.path(), vec![item(1, &a), item(2, &b)], None, |e| {
        e.set_crossfade(crossfade(1_000))
    });

    // The anchor published with the incoming track's start belongs to it: its position counts
    // from its own start and its duration is its own, while the outgoing track still sounds.
    let started = render
        .events
        .iter()
        .position(|e| matches!(e, EngineEvent::TrackStarted { id: 2, .. }))
        .unwrap();
    let ended = render
        .events
        .iter()
        .position(|e| matches!(e, EngineEvent::TrackEnded { id: 1, .. }))
        .unwrap();
    assert!(
        started < ended,
        "the incoming track starts before the outgoing one ends"
    );
    let anchors: Vec<_> = render.events[started..ended]
        .iter()
        .filter_map(|e| match e {
            EngineEvent::Anchor(a) => Some(*a),
            _ => None,
        })
        .collect();
    let first = anchors.first().expect("an anchor at the incoming track's start");
    println!(
        "anchor at the incoming start: position {} us, duration {:?}",
        first.position_us, first.duration_us
    );
    assert_eq!(first.duration_us, Some(2_500_000));
    assert!((0..30_000).contains(&first.position_us), "{}", first.position_us);
    assert!(anchors.iter().all(|a| a.duration_us == Some(2_500_000)));
}

#[test]
fn album_continuity_joins_tracks_of_one_album_gaplessly() {
    let dir = tempfile::tempdir().unwrap();
    let (freq, amplitude) = (1_000.0, 0.5);
    let (split, total) = (30_001, 66_000);
    let first = dir.path().join("part1.flac");
    let second = dir.path().join("part2.flac");
    let mut expected = sine_flac(&first, freq, RATE, 0, split, amplitude);
    expected.extend(sine_flac(&second, freq, RATE, split, total - split, amplitude));
    let album = |id, path: &Path| PlayItem::new(id, path).with_album("album-key");

    let joined = render(dir.path(), vec![album(1, &first), album(2, &second)], None, |e| {
        e.set_crossfade(crossfade(1_000))
    });
    assert_eq!(joined.frames(), total);
    assert_eq!(joined.left(), expected, "a sample-exact join, no crossfade");
    assert_eq!(
        track_events(&joined.events),
        ["start 0:1", "end 0:1", "start 1:2", "end 1:2", "queue end"]
    );

    // Without continuity the same pair crossfades. Both parts are under 2 s, so the overlap is
    // half of the shorter one (the first, 30 001 frames).
    let sub = dir.path().join("off");
    std::fs::create_dir(&sub).unwrap();
    let faded = render(&sub, vec![album(1, &first), album(2, &second)], None, |e| {
        e.set_crossfade(CrossfadeSettings {
            album_continuity: false,
            ..crossfade(1_000)
        })
    });
    let overlap = split / 2;
    println!(
        "continuity off: {} frames ({} overlapped)",
        faded.frames(),
        total - faded.frames()
    );
    assert_eq!(faded.frames(), total - overlap);
}

#[test]
fn a_track_shorter_than_the_fade_keeps_its_middle() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b, c) = (
        dir.path().join("a.wav"),
        dir.path().join("b.wav"),
        dir.path().join("c.wav"),
    );
    dc_wav(&a, 4.0, true);
    dc_wav(&b, 2.0, false);
    dc_wav(&c, 4.0, true);

    // 3 s asked, but B is 2 s: each of its transitions overlaps half of it (1 s), so at most two
    // tracks ever sound together.
    let render = render(
        dir.path(),
        vec![item(1, &a), item(2, &b), item(3, &c)],
        None,
        |e| e.set_crossfade(crossfade(3_000)),
    );

    let second = RATE as usize;
    assert_eq!(render.frames(), 8 * second, "4 + 2 + 4 s with two 1 s overlaps");
    let samples = &render.samples;
    check_overlap("A into B", samples, 3 * second, second, true);
    check_overlap("B into C", samples, 4 * second, second, false);
    assert_eq!(
        track_events(&render.events),
        [
            "start 0:1",
            "start 1:2",
            "end 0:1",
            "start 2:3",
            "end 1:2",
            "end 2:3",
            "queue end"
        ]
    );
}

#[test]
fn a_crossfade_is_shorter_when_the_outgoing_track_is() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.wav"), dir.path().join("b.wav"));
    dc_wav(&a, 1.5, true);
    dc_wav(&b, 6.0, false);

    let render = render(dir.path(), vec![item(1, &a), item(2, &b)], None, |e| {
        e.set_crossfade(crossfade(12_000))
    });
    let overlap = (0.75 * f64::from(RATE)) as usize;
    assert_eq!(render.frames(), (7.5 * f64::from(RATE)) as usize - overlap);
    let start = (0.75 * f64::from(RATE)) as usize;
    check_overlap("half of a 1.5 s track", &render.samples, start, overlap, true);
}

#[test]
fn a_manual_skip_neither_crossfades_nor_clicks() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.wav"), dir.path().join("b.wav"));
    sine_wav(&a, 440.0, RATE, 4 * RATE as usize, 0.5);
    sine_wav(&b, 660.0, RATE, RATE as usize, 0.5);

    // Offline: the engine fades out what it had not written yet and fades the new track in.
    let render = render_gated(dir.path(), vec![item(1, &a), item(2, &b)], 30_123, |engine| {
        engine.set_crossfade(crossfade(3_000));
        engine.next();
    });
    let left = render.left();
    let bound = sine_step_bound(660.0, RATE, 0.5);
    let (step, at) = max_step(&left);
    println!(
        "offline skip: {} frames, largest step {step:.4} at {at} (sine bound {bound:.4})",
        left.len()
    );
    assert!(step <= bound, "a click: {step} at {at}");
    // No overlap: after the skip only B sounds, and all of it.
    let skip_at = left.len() - RATE as usize;
    assert!((30_123..30_123 + 2_048).contains(&skip_at), "{skip_at}");
    let a_after = tone_amplitude(&left[skip_at + 441..skip_at + 441 + 4_410], 440.0, RATE);
    println!("440 Hz after the skip: {a_after:.2e}");
    assert!(a_after < 1e-3, "the old track goes on: {a_after}");
    assert_eq!(render.started(), [1, 2]);

    // A paced output: the sink fades out what it still held.
    let out = dir.path().join("paced.wav");
    let config = EngineConfig {
        output: OutputKind::WavFile {
            path: out.clone(),
            realtime: true,
        },
        buffer_ms: 200,
        ..EngineConfig::default()
    };
    let engine = Engine::new(config).unwrap();
    engine.set_crossfade(crossfade(3_000));
    engine.set_queue(vec![item(1, &a), item(2, &b)], 0);
    engine.play();
    std::thread::sleep(Duration::from_millis(600));
    engine.next();
    assert!(engine.wait_until_idle(IDLE_TIMEOUT));
    drop(engine);
    let (_, samples) = read_wav(&out);
    let left: Vec<f32> = samples.iter().step_by(2).copied().collect();
    let (step, at) = max_step(&left);
    println!(
        "paced skip: {} frames, largest step {step:.4} at {at} (sine bound {bound:.4})",
        left.len()
    );
    assert!(step <= bound, "a click: {step} at {at}");
    let skip_at = left.len() - RATE as usize;
    let a_after = tone_amplitude(&left[skip_at + 441..skip_at + 441 + 4_410], 440.0, RATE);
    assert!(a_after < 1e-3, "the old track goes on: {a_after}");
}

#[test]
fn a_seek_while_playing_does_not_click() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    sine_wav(&a, 440.0, RATE, 3 * RATE as usize, 0.5);
    let render = render_gated(dir.path(), vec![item(1, &a)], 20_000, |engine| engine.seek(2_000));
    let left = render.left();
    let bound = sine_step_bound(440.0, RATE, 0.5);
    let (step, at) = max_step(&left);
    println!(
        "seek while playing: {} frames, largest step {step:.4} at {at} (sine bound {bound:.4})",
        left.len()
    );
    assert!(step <= bound * 1.01, "a click: {step} at {at}");
    // What was written before the seek, a 10 ms fade-out, then the last second.
    let before = left.len() - RATE as usize;
    assert!((20_000 + 441..20_000 + 1_024 + 441).contains(&before), "{before}");
}
