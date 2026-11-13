//! `Engine::anchor()` is never older than an `Anchor` event that has been
//! received. The core answers each event by reading `engine.anchor()`; when
//! the engine stored the anchor only after sending the event, a seek while
//! paused sometimes showed the previous position in every app.

mod common;

use std::time::{Duration, Instant};

use common::*;
use jf_engine::{Engine, EngineConfig, EngineEvent, OutputKind};

#[test]
fn the_stored_anchor_is_never_older_than_an_anchor_event() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    sine_wav(&a, 440.0, 44_100, 10 * 44_100, 0.5);
    let engine = Engine::new(EngineConfig {
        output: OutputKind::Null { realtime: true },
        ..EngineConfig::default()
    })
    .unwrap();
    let events = engine.events();
    engine.set_queue(vec![item(1, &a)], 0);
    engine.play();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(
        events.recv_deadline(deadline).expect("the track starts"),
        EngineEvent::TrackStarted { .. }
    ) {}
    engine.pause();

    let mut checked = 0;
    for i in 0..200_u64 {
        let target_ms = 500 + (i * 83) % 9_000;
        engine.seek(target_ms);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let event = events
                .recv_deadline(deadline)
                .unwrap_or_else(|_| panic!("seek {i}: no anchor for {target_ms} ms"));
            if let EngineEvent::Anchor(anchor) = event {
                // Read right away, as the core does: it must be this anchor or a newer one.
                let stored = engine.anchor();
                assert!(
                    stored.anchor_mono_ns >= anchor.anchor_mono_ns
                        && stored.track_generation >= anchor.track_generation,
                    "seek {i} to {target_ms} ms: the stored anchor {stored:?} is older than the event's {anchor:?}"
                );
                checked += 1;
                // The paused anchor at the new position ends this seek (within a sample or so).
                if anchor.rate == 0.0 && (anchor.position_us - target_ms as i64 * 1_000).abs() <= 2_000 {
                    break;
                }
            }
        }
    }
    assert!(checked >= 200, "{checked} anchors checked");
}
