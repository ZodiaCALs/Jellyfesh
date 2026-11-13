//! Writes and checks `tests/contract/display-clock.json`: step-by-step runs of
//! `jf_core::DisplayClock` (updates, drawn positions, waits) and `next_second_us` /
//! `next_step_us` values, recorded from the Rust implementation. Every shell's port of the
//! drawing-time helpers replays it (`platforms/shared/`). To accept a deliberate change, run
//! `UPDATE_GOLDEN=1 cargo test -p jf-ffi --test drawing_contract` and review the diff.

use std::path::Path;

use jf_core::timing::{next_second_us, next_step_us};
use jf_core::{DisplayClock, PlaybackAnchor};
use serde_json::{Value, json};

const MS: u64 = 1_000_000; // nanoseconds

/// Records a run: each step applies to `clock` and stores what it returned.
struct Run {
    clock: DisplayClock,
    steps: Vec<Value>,
}

impl Run {
    fn new() -> Self {
        Run {
            clock: DisplayClock::new(),
            steps: Vec::new(),
        }
    }

    fn update(&mut self, anchor: PlaybackAnchor, now_ns: u64) {
        self.clock.update(anchor, now_ns);
        self.steps.push(json!({
            "op": "update",
            "now_ns": now_ns,
            "anchor": {
                "track_generation": anchor.track_generation,
                "position_us": anchor.position_us,
                "anchor_mono_ns": anchor.anchor_mono_ns,
                "rate": anchor.rate,
                "duration_us": anchor.duration_us,
            },
        }));
    }

    fn position(&mut self, now_ns: u64) -> i64 {
        let position = self.clock.position_us(now_ns);
        self.steps
            .push(json!({ "op": "position", "now_ns": now_ns, "expect": position }));
        position
    }

    fn wait_until(&mut self, target_us: i64, now_ns: u64) {
        let wait = self
            .clock
            .wait_until(target_us, now_ns)
            .map(|d| d.as_micros() as u64);
        self.steps.push(json!({
            "op": "wait_until", "target_us": target_us, "now_ns": now_ns, "expect_us": wait,
        }));
    }

    fn running(&mut self) {
        let running = self.clock.is_running();
        self.steps.push(json!({ "op": "running", "expect": running }));
    }

    fn case(self, name: &str) -> Value {
        json!({ "name": name, "steps": self.steps })
    }
}

fn anchor(generation: u64, position_ms: i64, at_ms: u64, rate: f32) -> PlaybackAnchor {
    PlaybackAnchor {
        track_generation: generation,
        position_us: position_ms * 1_000,
        anchor_mono_ns: at_ms * MS,
        rate,
        duration_us: Some(240_000_000),
    }
}

/// A deterministic stand-in for randomness: the same numbers on every run.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

fn cases() -> Vec<Value> {
    let mut cases = Vec::new();

    // The cases of timing.rs's unit tests.
    let mut r = Run::new();
    r.position(0);
    r.running();
    r.update(anchor(1, 10_000, 1_000, 1.0), 1_000 * MS);
    r.position(1_500 * MS);
    r.running();
    cases.push(r.case("follows the anchor"));

    let mut r = Run::new();
    r.update(anchor(1, 0, 0, 1.0), 0);
    r.position(1_000 * MS);
    r.update(anchor(1, 1_015, 1_000, 1.0), 1_000 * MS);
    for t in (1_000..=1_200).step_by(5) {
        r.position(t * MS);
    }
    cases.push(r.case("small corrections are slewed, not jumped"));

    let mut r = Run::new();
    r.update(anchor(1, 0, 0, 1.0), 0);
    r.position(2_000 * MS);
    r.update(anchor(1, 1_980, 2_000, 1.0), 2_000 * MS);
    for t in (2_000..=2_200).step_by(4) {
        r.position(t * MS);
    }
    cases.push(r.case("a backward correction never moves backwards"));

    let mut r = Run::new();
    r.update(anchor(1, 0, 0, 1.0), 0);
    r.position(5_000 * MS);
    r.update(anchor(2, 1_000, 5_000, 1.0), 5_000 * MS);
    r.position(5_000 * MS);
    r.update(anchor(2, 1_300, 5_300, 0.0), 5_300 * MS);
    r.position(9_000 * MS);
    r.running();
    r.wait_until(2_000_000, 9_000 * MS);
    r.update(anchor(2, 1_300, 9_000, 1.0), 9_000 * MS);
    r.update(anchor(2, 3_000, 9_100, 1.0), 9_100 * MS);
    r.position(9_100 * MS);
    cases.push(r.case("seeks, pauses and new tracks show at once"));

    let mut r = Run::new();
    r.update(anchor(1, 239_900, 0, 1.0), 0);
    r.position(1_000 * MS);
    r.update(anchor(2, -50, 0, 1.0), 0);
    r.position(0);
    cases.push(r.case("stays within the track"));

    let mut r = Run::new();
    r.update(anchor(1, 1_250, 0, 1.0), 0);
    r.wait_until(2_000_000, 0);
    r.wait_until(1_000_000, 0);
    r.update(anchor(2, 0, 0, 2.0), 0);
    r.wait_until(1_000_000, 0);
    r.wait_until(1_000_001, 0);
    cases.push(r.case("waits until a target"));

    // Sub-microsecond extrapolation rounds half away from zero, also before the anchor.
    let mut r = Run::new();
    r.update(
        PlaybackAnchor {
            track_generation: 1,
            position_us: 1_000_000,
            anchor_mono_ns: 5_000_000_000,
            rate: 1.0,
            duration_us: None,
        },
        5_000_000_000,
    );
    for now in [5_000_000_499, 5_000_000_500, 5_000_001_500, 5_000_002_500] {
        r.position(now);
    }
    r.update(
        PlaybackAnchor {
            track_generation: 2,
            position_us: 3_000_000,
            anchor_mono_ns: 3_000_000_000,
            rate: 0.5,
            duration_us: None,
        },
        2_000_000_000,
    );
    r.position(2_000_000_000);
    r.position(2_000_000_999);
    r.wait_until(4_000_000, 2_000_000_000);
    cases.push(r.case("rounding and other rates"));

    // A long, deterministic run: re-anchors with small and large drifts (both directions),
    // pauses, resumes, seeks and track changes, drawn every 11 ms.
    let mut r = Run::new();
    let mut lcg = Lcg(0x5eed);
    let mut generation = 1;
    let mut playing = true;
    // The engine's clock: `base_ms` at `base_t` (ms), advancing while playing.
    let (mut base_t, mut base_ms): (u64, i64) = (0, 0);
    let mut t: u64 = 0;
    r.update(anchor(generation, 0, 0, 1.0), 0);
    for second in 1..=24u64 {
        let next = second * 1_000;
        while t + 11 < next {
            t += 11;
            r.position(t * MS);
        }
        t = next;
        let now_ms = if playing {
            base_ms + (t - base_t) as i64
        } else {
            base_ms
        };
        let drift = lcg.below(41) as i64 - 20; // −20 … +20 ms
        let position = match lcg.below(10) {
            0 => {
                generation += 1;
                lcg.below(200_000) as i64
            }
            1 => {
                playing = !playing;
                now_ms
            }
            2 => now_ms + if drift < 0 { -300 } else { 300 },
            _ => now_ms + drift,
        };
        (base_t, base_ms) = (t, position);
        r.update(
            anchor(generation, position, t, if playing { 1.0 } else { 0.0 }),
            t * MS,
        );
        r.position(t * MS);
        r.wait_until(next_second_us(position * 1_000), t * MS);
        r.running();
    }
    cases.push(r.case("a long run with drift, seeks and pauses"));
    cases
}

/// The file text: one step or value row per line, so it stays small and diffs well.
fn build() -> String {
    let value = contract();
    let compact = |v: &Value| serde_json::to_string(v).unwrap();
    let mut out = String::from("{\n");
    out += &format!("  \"about\": {},\n", compact(&value["about"]));
    out += &format!("  \"version\": {},\n", value["version"]);
    for key in ["next_second", "next_step"] {
        out += &format!("  \"{key}\": [\n");
        let rows = value[key].as_array().unwrap();
        for (i, row) in rows.iter().enumerate() {
            let comma = if i + 1 == rows.len() { "" } else { "," };
            out += &format!("    {}{comma}\n", compact(row));
        }
        out += "  ],\n";
    }
    out += "  \"cases\": [\n";
    let cases = value["cases"].as_array().unwrap();
    for (i, case) in cases.iter().enumerate() {
        out += &format!("    {{\"name\": {}, \"steps\": [\n", compact(&case["name"]));
        let steps = case["steps"].as_array().unwrap();
        for (j, step) in steps.iter().enumerate() {
            let comma = if j + 1 == steps.len() { "" } else { "," };
            out += &format!("      {}{comma}\n", compact(step));
        }
        let comma = if i + 1 == cases.len() { "" } else { "," };
        out += &format!("    ]}}{comma}\n");
    }
    out += "  ]\n}\n";
    out
}

fn contract() -> Value {
    let positions = [
        i64::MIN + 1,
        -2_500_001,
        -1_000_000,
        -1,
        0,
        1,
        999_999,
        1_000_000,
        1_250_000,
        59_999_999,
        i64::MAX / 2,
    ];
    let next_second: Vec<Value> = positions
        .iter()
        .filter(|p| p.checked_add(1_000_000).is_some())
        .map(|&p| json!([p, next_second_us(p)]))
        .collect();
    let mut next_step = Vec::new();
    for &p in &positions[1..positions.len() - 1] {
        for step in [0, -5, 1, 7, 400_000, 1_000_000] {
            next_step.push(json!([p, step, next_step_us(p, step)]));
        }
    }
    json!({
        "about": "Drawing-time contract, recorded from jf_core::DisplayClock, next_second_us and next_step_us \
                  (core/jf-core/src/timing.rs). Each case starts a new DisplayClock and applies its steps in \
                  order: update(anchor, now_ns); position(now_ns) must return expect (µs); wait_until(target_us, \
                  now_ns) must return expect_us (µs, null for none); running must equal expect. next_second \
                  rows are [position_us, expected]; next_step rows are [position_us, step_us, expected or null]. \
                  Generated by core/jf-ffi/tests/drawing_contract.rs; every shell's port replays it.",
        "version": 1,
        "cases": cases(),
        "next_second": next_second,
        "next_step": next_step,
    })
}

#[test]
fn drawing_contract_is_current() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/contract/display-clock.json");
    let fresh = build();
    let parsed: Value = serde_json::from_str(&fresh).unwrap();
    assert_eq!(parsed, contract(), "the file text is the contract");
    if std::env::var("UPDATE_GOLDEN").is_ok_and(|v| v == "1") {
        std::fs::write(&path, &fresh).unwrap();
        return;
    }
    let stored = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (run with UPDATE_GOLDEN=1 to create it)", path.display()));
    assert!(
        stored.replace("\r\n", "\n") == fresh,
        "{} is out of date: run UPDATE_GOLDEN=1 cargo test -p jf-ffi --test drawing_contract and review the diff",
        path.display()
    );
}
