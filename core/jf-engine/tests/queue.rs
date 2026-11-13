//! Queue behaviour: gapless joins, set_upcoming, errors, seeking and the anchor.

mod common;

use std::time::Duration;

use common::*;
use jf_engine::{Engine, EngineConfig, EngineEvent, OutputKind, PlayItem, monotonic_now_ns};

/// Waits for the next `TrackStarted` event and returns its id.
fn next_started(events: &crossbeam_channel::Receiver<EngineEvent>) -> u64 {
    loop {
        match events
            .recv_timeout(Duration::from_secs(30))
            .expect("no TrackStarted event")
        {
            EngineEvent::TrackStarted { id, .. } => return id,
            EngineEvent::Error { message, .. } => panic!("engine error: {message}"),
            _ => {}
        }
    }
}

/// Track events (started/ended/queue end) as compact strings, in order.
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

#[test]
fn gapless_flac_join_at_a_non_block_boundary_is_continuous() {
    let dir = tempfile::tempdir().unwrap();
    let (freq, rate, amplitude) = (1_000.0, 44_100, 0.5);
    // One continuous sine split after 30 001 frames: not a multiple of FLAC's 4 096-frame blocks.
    let (split, total) = (30_001, 66_000);
    let first = dir.path().join("part1.flac");
    let second = dir.path().join("part2.flac");
    let mut expected = sine_flac(&first, freq, rate, 0, split, amplitude);
    expected.extend(sine_flac(&second, freq, rate, split, total - split, amplitude));

    let render = render(dir.path(), vec![item(1, &first), item(2, &second)], None, |_| {});

    assert_eq!(render.frames(), total, "no frames lost or added at the join");
    let left = render.left();
    assert_eq!(left, expected, "the join is sample-exact");
    let bound = sine_step_bound(freq, rate, amplitude) + 2.0 / 32_768.0;
    let (join_step, _) = max_step(&left[split - 1..split + 1]);
    let (worst, at) = max_step(&left);
    println!("gapless FLAC 1 kHz: join step {join_step:.5}, max step {worst:.5} at {at}, bound {bound:.5}");
    assert!(join_step <= bound, "{join_step} > {bound}");
    assert!(worst <= bound, "{worst} at {at} > {bound}");
    assert_eq!(
        track_events(&render.events),
        ["start 0:1", "end 0:1", "start 1:2", "end 1:2", "queue end"]
    );
    let gapless = render.events.iter().all(|e| match e {
        EngineEvent::TrackStarted { info, .. } => info.gapless && info.codec == "flac",
        _ => true,
    });
    assert!(gapless);
}

#[test]
fn gapless_join_stays_continuous_through_the_resampler() {
    let dir = tempfile::tempdir().unwrap();
    let (freq, amplitude) = (1_000.0, 0.5);
    let (split, total) = (33_333, 72_000); // at 48 kHz
    let first = dir.path().join("part1.flac");
    let second = dir.path().join("part2.flac");
    sine_flac(&first, freq, 48_000, 0, split, amplitude);
    sine_flac(&second, freq, 48_000, split, total - split, amplitude);

    let render = render(
        dir.path(),
        vec![item(1, &first), item(2, &second)],
        Some(44_100),
        |_| {},
    );

    let expected_frames = (total * 44_100 + 24_000) / 48_000;
    assert_eq!(render.frames(), expected_frames);
    let left = render.left();
    let join = split * 44_100 / 48_000;
    let bound = sine_step_bound(freq, 44_100, amplitude) * 1.01 + 2.0 / 32_768.0;
    let (join_step, _) = max_step(&left[join - 500..join + 500]);
    // The first and last few frames ring where the sine starts and stops abruptly (band-limited
    // edges of the files, not of the join), so the whole-file check leaves them out.
    let (worst, at) = max_step(&left[200..left.len() - 200]);
    println!(
        "gapless 48 -> 44.1 kHz: steps near the join {join_step:.5}, max {worst:.5} at {at}, bound {bound:.5}"
    );
    assert!(join_step <= bound, "{join_step} > {bound}");
    assert!(worst <= bound, "{worst} at {at} > {bound}");
}

#[test]
fn corrupt_and_missing_files_are_skipped_with_errors() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    let b = dir.path().join("b.wav");
    sine_wav(&a, 440.0, 44_100, 22_050, 0.5);
    sine_wav(&b, 660.0, 44_100, 22_050, 0.5);
    let corrupt = dir.path().join("corrupt.flac");
    let mut state = 0x1234_5678u32;
    let garbage: Vec<u8> = (0..20_000)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        })
        .collect();
    std::fs::write(&corrupt, garbage).unwrap();
    let missing = dir.path().join("missing.wav");

    let items = vec![item(1, &a), item(2, &corrupt), item(3, &missing), item(4, &b)];
    let render = render(dir.path(), items, None, |_| {});

    assert_eq!(render.errors(), [Some(2), Some(3)]);
    assert_eq!(render.started(), [1, 4]);
    assert_eq!(render.frames(), 44_100);
    let left = render.left();
    let (f1, f2) = (
        zero_crossing_frequency(&left[..22_050], 44_100),
        zero_crossing_frequency(&left[22_050..], 44_100),
    );
    println!("skip over corrupt files: {f1:.2} Hz then {f2:.2} Hz");
    assert!((f1 - 440.0).abs() < 0.5 && (f2 - 660.0).abs() < 0.5);
}

/// A 16-bit "position code": every frame's value names its index modulo 32 768.
fn position_code_flac(path: &std::path::Path, frames: usize) {
    let samples: Vec<i32> = (0..frames)
        .flat_map(|n| [(n % 32_768) as i32 - 16_384; 2])
        .collect();
    write_flac16(path, 44_100, 2, &samples);
}

fn decode_position(sample: f32) -> usize {
    ((sample * 32_768.0).round() as i64 + 16_384) as usize
}

#[test]
fn seek_lands_on_the_target_frame() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("code.flac");
    let frames = 132_300; // 3 s
    position_code_flac(&path, frames);

    // Seek before playing.
    let render = render(dir.path(), vec![item(1, &path)], None, |e| e.seek(1_500));
    let target = 66_150;
    let landed = decode_position(render.left()[0]);
    println!(
        "seek 1500 ms from stopped: landed on frame code {landed} (target {})",
        target % 32_768
    );
    assert_eq!(
        landed,
        target % 32_768,
        "within one block is required; this is exact"
    );
    assert_eq!(render.frames(), frames - target);

    // Seek while paused part-way through.
    let sub = dir.path().join("paused");
    std::fs::create_dir(&sub).unwrap();
    let render = render_with(
        &sub,
        vec![item(1, &path)],
        None,
        |_| {},
        |engine, _| {
            engine.pause();
            assert!(engine.wait_until_idle(IDLE_TIMEOUT));
            engine.seek(2_000);
            engine.play();
        },
    );
    let left = render.left();
    let jump = (0..left.len())
        .find(|&i| decode_position(left[i]) != i % 32_768)
        .unwrap_or(left.len());
    let target = 88_200;
    let landed = decode_position(left[jump]);
    println!(
        "seek 2000 ms while paused after {jump} frames: landed on frame code {landed} (target {})",
        target % 32_768
    );
    assert_eq!(landed, target % 32_768);
    assert_eq!(left.len() - jump, frames - target);
}

#[test]
fn set_upcoming_while_playing_changes_what_follows() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b, c) = (
        dir.path().join("a.wav"),
        dir.path().join("b.wav"),
        dir.path().join("c.wav"),
    );
    sine_wav(&a, 440.0, 44_100, 4 * 44_100, 0.5);
    sine_wav(&b, 880.0, 44_100, 44_100, 0.5);
    sine_wav(&c, 660.0, 44_100, 44_100, 0.5);

    // The engine thread is held one second into A while the upcoming items change.
    let render = render_gated(dir.path(), vec![item(1, &a), item(2, &b)], 44_100, |engine| {
        engine.set_upcoming(vec![item(3, &c)]);
    });

    assert_eq!(render.frames(), 5 * 44_100, "A then C, without B");
    let left = render.left();
    let tail = &left[4 * 44_100..];
    let freq = zero_crossing_frequency(tail, 44_100);
    let b_level = tone_amplitude(tail, 880.0, 44_100);
    println!("set_upcoming: second tone {freq:.2} Hz, 880 Hz residue {b_level:.1e}");
    assert!((freq - 660.0).abs() < 0.5, "{freq}");
    assert!(b_level < 1e-3);
    assert_eq!(render.started(), [1, 3]);
    assert_eq!(
        track_events(&render.events),
        ["start 0:1", "end 0:1", "start 1:3", "end 1:3", "queue end"]
    );
}

#[test]
fn set_upcoming_with_the_same_item_loops_it_gaplessly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loop.flac");
    // 441 Hz at 44.1 kHz: exactly 100 frames per cycle, so the loop point is seamless.
    let (freq, amplitude, frames) = (441.0, 0.5, 3 * 44_100);
    let one = sine_flac(&path, freq, 44_100, 0, frames, amplitude);

    let render = render_gated(dir.path(), vec![item(7, &path)], 44_100, |engine| {
        engine.set_upcoming(vec![item(7, &path)]);
    });

    assert_eq!(render.frames(), 2 * frames);
    let left = render.left();
    assert_eq!(&left[..frames], &one[..]);
    assert_eq!(&left[frames..], &one[..], "the repeat starts from the beginning");
    let bound = sine_step_bound(freq, 44_100, amplitude) + 2.0 / 32_768.0;
    let (join_step, _) = max_step(&left[frames - 1..frames + 1]);
    let (worst, _) = max_step(&left);
    println!("repeat-one loop 441 Hz: join step {join_step:.5}, max step {worst:.5}, bound {bound:.5}");
    assert!(join_step <= bound && worst <= bound);
    assert_eq!(
        track_events(&render.events),
        ["start 0:7", "end 0:7", "start 1:7", "end 1:7", "queue end"]
    );
}

#[test]
fn set_upcoming_after_the_next_track_was_buffered_rerenders_the_join() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b, c) = (
        dir.path().join("a.wav"),
        dir.path().join("b.wav"),
        dir.path().join("c.wav"),
    );
    let a_samples = stereo(&sine(440.0, 44_100, 0, 44_100, 0.5));
    let c_samples = stereo(&sine(660.0, 44_100, 0, 44_100, 0.5));
    write_wav_f32(&a, 44_100, 2, &a_samples);
    sine_wav(&b, 880.0, 44_100, 44_100, 0.5);
    write_wav_f32(&c, 44_100, 2, &c_samples);

    // 8 192 frames stay buffered "in the device"; stop the engine thread 2 048 frames before A
    // ends, when the start of B has already been written to the output.
    let (mut sink, played) = LatencySink::new(44_100, 8_192);
    let (reached, release) = sink.gate_at(44_100 - 2_048);
    let engine = Engine::with_sink(Box::new(sink)).unwrap();
    let events = engine.events();
    engine.set_queue(vec![item(1, &a), item(2, &b)], 0);
    engine.play();
    reached
        .recv_timeout(Duration::from_secs(30))
        .expect("the gate was not reached");
    engine.set_upcoming(vec![item(3, &c)]);
    release.send(()).unwrap();
    assert!(engine.wait_until_idle(IDLE_TIMEOUT));
    drop(engine);

    let played = played.lock().clone();
    let events: Vec<EngineEvent> = events.try_iter().collect();
    assert_eq!(played.len(), 2 * 2 * 44_100, "A then C, nothing of B");
    let a_exact = played[..a_samples.len()]
        .iter()
        .zip(&a_samples)
        .all(|(&x, &y)| x == y as f32);
    let c_exact = played[a_samples.len()..]
        .iter()
        .zip(&c_samples)
        .all(|(&x, &y)| x == y as f32);
    assert!(a_exact, "A is complete and continuous across the re-render");
    assert!(c_exact, "C follows A directly");
    assert_eq!(
        track_events(&events),
        ["start 0:1", "end 0:1", "start 1:3", "end 1:3", "queue end"]
    );
}

/// A, B and C of one second each, distinguishable by pitch.
fn three_tones(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let (a, b, c) = (dir.join("a.wav"), dir.join("b.wav"), dir.join("c.wav"));
    sine_wav(&a, 440.0, 44_100, 44_100, 0.5);
    sine_wav(&b, 880.0, 44_100, 44_100, 0.5);
    sine_wav(&c, 660.0, 44_100, 44_100, 0.5);
    (a, b, c)
}

#[test]
fn set_upcoming_after_a_track_already_left_does_not_queue_its_follower_twice() {
    // The caller heard that A started and sends "after A comes B", but playback is already
    // in B when the call arrives. Appending after the audible item would play B twice.
    let dir = tempfile::tempdir().unwrap();
    let (a, b, _) = three_tones(dir.path());
    let render = render_gated(
        dir.path(),
        vec![item(1, &a), item(2, &b)],
        44_100 + 22_050,
        |engine| {
            engine.set_upcoming_after(1, vec![item(2, &b)]);
        },
    );
    assert_eq!(render.frames(), 2 * 44_100, "A then B, once");
    assert_eq!(
        track_events(&render.events),
        ["start 0:1", "end 0:1", "start 1:2", "end 1:2", "queue end"]
    );
}

#[test]
fn a_stale_set_upcoming_after_is_ignored() {
    // "After A comes C" arrives while B (which followed A) plays: the caller has not heard
    // of B yet and will send a newer list; this one must not cut B short or follow it.
    let dir = tempfile::tempdir().unwrap();
    let (a, b, c) = three_tones(dir.path());
    let render = render_gated(
        dir.path(),
        vec![item(1, &a), item(2, &b)],
        44_100 + 22_050,
        |engine| {
            engine.set_upcoming_after(1, vec![item(3, &c)]);
        },
    );
    assert_eq!(render.frames(), 2 * 44_100);
    assert_eq!(render.started(), [1, 2]);
}

#[test]
fn set_upcoming_after_the_audible_track_works_like_set_upcoming() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b, c) = three_tones(dir.path());
    let render = render_gated(dir.path(), vec![item(1, &a), item(2, &b)], 22_050, |engine| {
        engine.set_upcoming_after(1, vec![item(3, &c)]);
    });
    assert_eq!(render.frames(), 2 * 44_100, "A then C");
    assert_eq!(render.started(), [1, 3]);
}

#[test]
fn set_upcoming_with_nothing_ends_after_the_current_item() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.wav"), dir.path().join("b.wav"));
    sine_wav(&a, 440.0, 44_100, 3 * 44_100, 0.5);
    sine_wav(&b, 880.0, 44_100, 44_100, 0.5);

    let render = render_gated(dir.path(), vec![item(1, &a), item(2, &b)], 44_100, |engine| {
        engine.set_upcoming(Vec::new());
    });

    assert_eq!(render.frames(), 3 * 44_100);
    assert_eq!(
        track_events(&render.events),
        ["start 0:1", "end 0:1", "queue end"]
    );
}

#[test]
fn anchor_follows_real_time_playback() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    sine_wav(&path, 440.0, 44_100, 5 * 44_100, 0.5);
    let config = EngineConfig {
        output: OutputKind::Null { realtime: true },
        // Longer than a busy CI machine stalls (macOS runners have stalled the whole test for
        // 150 ms). A stall longer than the buffer runs the device dry, and until the engine's next
        // drift check the anchor is ahead of the sound: right, but not what this test measures.
        buffer_ms: 250,
        ..EngineConfig::default()
    };
    let engine = Engine::new(config).unwrap();
    let events = engine.events();
    engine.set_queue(vec![PlayItem::new(1, &path)], 0);
    engine.play();
    assert_eq!(next_started(&events), 1);
    let started = monotonic_now_ns();

    std::thread::sleep(Duration::from_millis(600));
    let anchor = engine.anchor();
    let elapsed_us = ((monotonic_now_ns() - started) / 1_000) as i64;
    let position = anchor.position_at(monotonic_now_ns());
    println!("playing: extrapolated {position} us after {elapsed_us} us");
    assert_eq!(anchor.rate, 1.0);
    assert!(
        (position - elapsed_us).abs() < 60_000,
        "{position} vs {elapsed_us}"
    );
    assert_eq!(anchor.duration_us, Some(5_000_000));

    engine.pause();
    assert!(engine.wait_until_idle(Duration::from_secs(5)));
    let paused = engine.anchor();
    let held = paused.position_at(monotonic_now_ns());
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(paused.rate, 0.0);
    assert_eq!(
        paused.position_at(monotonic_now_ns()),
        held,
        "a paused anchor holds still"
    );
    // Paused within a command's latency of the last reading.
    assert!(
        held >= position - 20_000 && held < position + 250_000,
        "{held} vs {position}"
    );

    engine.seek(2_000);
    assert!(engine.wait_until_idle(Duration::from_secs(5)));
    let seeked = engine.anchor();
    assert_eq!(seeked.position_us, 2_000_000);
    assert!(seeked.track_generation > paused.track_generation);

    // Measured rather than trusting the sleep, which may overshoot on a busy machine.
    let resumed_at = monotonic_now_ns();
    engine.play();
    std::thread::sleep(Duration::from_millis(300));
    let now = monotonic_now_ns();
    let position = engine.anchor().position_at(now);
    let expected = 2_000_000 + ((now - resumed_at) / 1_000) as i64;
    println!(
        "after seek to 2 s and {} us of play: {position} us",
        expected - 2_000_000
    );
    assert!((position - expected).abs() < 60_000, "{position} vs {expected}");

    engine.stop();
    assert!(engine.wait_until_idle(Duration::from_secs(5)));
    let stopped = engine.anchor();
    assert_eq!((stopped.rate, stopped.position_us), (0.0, 0));
}

/// A sink whose output dies after a few writes, like an unplugged device.
struct DyingSink {
    writes: usize,
}

impl jf_engine::Sink for DyingSink {
    fn sample_rate(&self) -> u32 {
        44_100
    }
    fn channels(&self) -> u16 {
        2
    }
    fn write(&mut self, _samples: &[f32]) -> Result<(), jf_engine::EngineError> {
        self.writes += 1;
        if self.writes > 4 {
            return Err(jf_engine::EngineError::Device("the device was unplugged".into()));
        }
        Ok(())
    }
    fn flush(&mut self) -> u64 {
        0
    }
    fn latency_us(&self) -> u64 {
        0
    }
}

#[test]
fn an_output_failure_is_reported_without_an_item_and_stops() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    sine_wav(&a, 440.0, 44_100, 44_100, 0.5);
    let engine = Engine::with_sink(Box::new(DyingSink { writes: 0 })).unwrap();
    let events = engine.events();
    engine.set_queue(vec![item(1, &a)], 0);
    engine.play();
    let message = loop {
        match events
            .recv_timeout(Duration::from_secs(30))
            .expect("no failure event")
        {
            EngineEvent::OutputFailed { message } => break message,
            EngineEvent::Error { id, message } => panic!("an error for {id:?} instead: {message}"),
            _ => {}
        }
    };
    assert!(message.contains("unplugged"), "{message}");
    assert!(
        engine.wait_until_idle(IDLE_TIMEOUT),
        "the engine stops instead of retrying forever"
    );
    assert_eq!(engine.anchor().rate, 0.0);
}

type SinkResult = Result<Box<dyn jf_engine::Sink>, jf_engine::EngineError>;

/// Sinks for an engine whose first output dies after a few writes; the next is a WAV file.
fn dying_then_wav(path: std::path::PathBuf) -> impl FnMut() -> SinkResult + Send + 'static {
    let mut sinks = 0;
    move || {
        sinks += 1;
        if sinks == 1 {
            Ok(Box::new(DyingSink { writes: 0 }))
        } else {
            Ok(Box::new(jf_engine::WavSink::new(&path, None)?))
        }
    }
}

#[test]
fn a_failed_output_stops_keeps_the_position_and_reopens_on_the_next_start() {
    let dir = tempfile::tempdir().unwrap();
    let code = dir.path().join("code.flac");
    position_code_flac(&code, 2 * 44_100);
    let out = dir.path().join("fresh.wav");
    let engine = Engine::with_sink_factory(dying_then_wav(out.clone())).unwrap();
    let events = engine.events();
    engine.set_queue(vec![item(1, &code)], 0);
    engine.play();
    assert!(
        engine.wait_until_idle(IDLE_TIMEOUT),
        "the engine stops instead of retrying forever"
    );
    let failed: Vec<EngineEvent> = events.try_iter().collect();
    let outputs: Vec<&String> = failed
        .iter()
        .filter_map(|e| match e {
            EngineEvent::OutputFailed { message } => Some(message),
            _ => None,
        })
        .collect();
    assert_eq!(outputs.len(), 1, "{failed:?}");
    assert!(
        !failed.iter().any(|e| matches!(e, EngineEvent::Error { .. })),
        "{failed:?}"
    );
    let kept = engine.anchor();
    println!("after the failure: {kept:?}");
    assert_eq!(kept.rate, 0.0);
    // Four 1024-frame writes reached the dying output: the position stays there.
    assert_eq!(kept.position_us, 4 * 1024 * 1_000_000 / 44_100);

    // Play resumes from the kept position on a fresh sink.
    engine.play();
    assert!(engine.wait_until_idle(IDLE_TIMEOUT));
    let resumed: Vec<EngineEvent> = events.try_iter().collect();
    assert!(
        resumed.iter().any(|e| matches!(e, EngineEvent::QueueEnded)),
        "{resumed:?}"
    );
    assert!(
        !resumed
            .iter()
            .any(|e| matches!(e, EngineEvent::OutputFailed { .. }))
    );
    drop(engine);
    let (_, samples) = read_wav(&out);
    let first = decode_position(samples[0]);
    let expected = (kept.position_us as u64 * 44_100 / 1_000_000) as usize % 32_768;
    println!("resumed at frame code {first} (kept position {expected})");
    assert!(first.abs_diff(expected) <= 1, "{first} vs {expected}");
}

#[test]
fn after_a_failed_output_a_new_queue_plays_on_a_fresh_sink() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    sine_wav(&a, 440.0, 44_100, 44_100, 0.5);
    let out = dir.path().join("fresh.wav");
    let engine = Engine::with_sink_factory(dying_then_wav(out.clone())).unwrap();
    let events = engine.events();
    engine.set_queue(vec![item(1, &a)], 0);
    engine.play();
    assert!(engine.wait_until_idle(IDLE_TIMEOUT));
    engine.set_queue(vec![item(2, &a)], 0);
    engine.play();
    assert!(engine.wait_until_idle(IDLE_TIMEOUT));
    drop(engine);
    let events: Vec<EngineEvent> = events.try_iter().collect();
    let failures = events
        .iter()
        .filter(|e| matches!(e, EngineEvent::OutputFailed { .. }))
        .count();
    assert_eq!(failures, 1, "{events:?}");
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, EngineEvent::QueueEnded))
            .count(),
        1,
        "{events:?}"
    );
    let (_, samples) = read_wav(&out);
    assert_eq!(
        samples.len(),
        2 * 44_100,
        "the new queue played completely on the fresh sink"
    );
}
