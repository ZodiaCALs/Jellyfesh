//! AAC in MP4 and Opus: gapless trimming (spikes S1 and S2), output gain, seeking and bit rate.
//!
//! The fixtures are short generated sines (see `fixtures/make-fixtures.sh`): each pair is one
//! continuous 1 kHz sine at amplitude 0.5 split at a frame that is not a multiple of the
//! codec's frame size, so the output is checked against the sine it was made from.

mod common;

use std::f64::consts::PI;
use std::path::PathBuf;

use common::*;
use jf_engine::{EngineEvent, TrackInfo};

/// Largest step allowed at a join of separately encoded lossy files, in multiples of the sine's
/// own largest step.
const LOSSY_STEP_ALLOWANCE: f64 = 1.6;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Where a 1 kHz sine in `x` (frames `first..`) lies against `0.5 sin(2π·1000·n/rate)`: a least
/// squares fit gives its amplitude and its offset in frames (positive = early).
fn sine_fit(x: &[f32], first: usize, rate: u32) -> (f64, f64) {
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
    ((a * a + b * b).sqrt(), b.atan2(a) / w)
}

fn started_info(events: &[EngineEvent]) -> Vec<TrackInfo> {
    events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::TrackStarted { info, .. } => Some(info.clone()),
            _ => None,
        })
        .collect()
}

/// Checks a decoded sine against the reference over windows; returns the worst offset.
fn check_alignment(label: &str, left: &[f32], first: usize, rate: u32, windows: &[(usize, usize)]) -> f64 {
    let mut worst: f64 = 0.0;
    for &(from, to) in windows {
        let (amplitude, offset) = sine_fit(&left[from..to], first + from, rate);
        println!("{label}: frames {from}..{to}: amplitude {amplitude:.4}, offset {offset:+.3} frames");
        assert!((amplitude - 0.5).abs() < 0.02, "{label}: amplitude {amplitude}");
        worst = worst.max(offset.abs());
    }
    worst
}

#[test]
fn aac_in_mp4_is_trimmed_to_its_exact_length() {
    let dir = tempfile::tempdir().unwrap();
    let render = render(dir.path(), vec![item(1, &fixture("aac-part1.m4a"))], None, |_| {});

    assert_eq!(render.rate, 44_100);
    assert_eq!(
        render.frames(),
        26_461,
        "priming and padding are gone, nothing else"
    );
    let left = render.left();
    // The first and last windows sit right at the trimmed edges.
    let worst = check_alignment(
        "aac part 1",
        &left,
        0,
        44_100,
        &[(0, 2_048), (10_000, 14_000), (24_413, 26_461)],
    );
    assert!(
        worst < 0.25,
        "a trim off by a frame would shift the sine by 1.0: {worst}"
    );
    let info = &started_info(&render.events)[0];
    println!("aac part 1: {info:?}");
    assert_eq!(
        (info.codec.as_str(), info.gapless, info.sample_rate),
        ("aac", true, 44_100)
    );
    assert_eq!(info.duration_us, Some(600_022));
    let bitrate = info.bitrate.expect("bit rate");
    assert!((50_000..90_000).contains(&bitrate), "{bitrate}");
}

#[test]
fn itunsmpb_trims_like_the_edit_list() {
    let dir = tempfile::tempdir().unwrap();
    let elst = render(dir.path(), vec![item(1, &fixture("aac-part1.m4a"))], None, |_| {});
    let sub = dir.path().join("smpb");
    std::fs::create_dir(&sub).unwrap();
    let smpb = render(&sub, vec![item(1, &fixture("aac-smpb.m4a"))], None, |_| {});

    assert_eq!(smpb.frames(), 26_461);
    assert_eq!(smpb.samples, elst.samples, "same audio, same trim");
    assert!(started_info(&smpb.events)[0].gapless);
}

#[test]
fn aac_parts_join_gaplessly() {
    let dir = tempfile::tempdir().unwrap();
    let items = vec![
        item(1, &fixture("aac-part1.m4a")),
        item(2, &fixture("aac-part2.m4a")),
    ];
    let render = render(dir.path(), items, None, |_| {});

    assert_eq!(render.frames(), 26_461 + 17_640);
    let left = render.left();
    let join = 26_461;
    let worst = check_alignment(
        "aac join",
        &left,
        0,
        44_100,
        &[
            (join - 2_048, join),
            (join, join + 2_048),
            (left.len() - 2_048, left.len()),
        ],
    );
    // Each file was encoded on its own, so the codec's noise is largest at the file edges (the
    // device check allows lossy codecs twice the sine's step). A gap or a misplaced frame shows
    // in the alignment above instead: one frame off is an offset of 1.0.
    let bound = sine_step_bound(1_000.0, 44_100, 0.5) * LOSSY_STEP_ALLOWANCE;
    let (step, at) = max_step(&left[join - 64..join + 64]);
    println!(
        "aac join: largest step {step:.4} at {} (sine step x{LOSSY_STEP_ALLOWANCE} {bound:.4})",
        join - 64 + at
    );
    assert!(step < bound, "{step}");
    assert!(worst < 0.25, "{worst}");
    assert!(started_info(&render.events).iter().all(|i| i.gapless));
}

#[test]
fn aac_seeks_land_on_the_sine() {
    let dir = tempfile::tempdir().unwrap();
    let render = render(dir.path(), vec![item(1, &fixture("aac-part1.m4a"))], None, |e| {
        e.seek(300)
    });
    let target = 13_230;
    assert_eq!(render.frames(), 26_461 - target);
    let worst = check_alignment("aac seek", &render.left(), target, 44_100, &[(0, 2_048)]);
    assert!(worst < 0.25, "{worst}");
}

#[test]
fn opus_is_trimmed_by_pre_skip_and_final_granule() {
    let dir = tempfile::tempdir().unwrap();
    let render = render(
        dir.path(),
        vec![item(1, &fixture("opus-part1.opus"))],
        None,
        |_| {},
    );

    assert_eq!(render.rate, 48_000);
    assert_eq!(render.frames(), 33_601, "pre-skip and end padding are gone");
    let left = render.left();
    let worst = check_alignment(
        "opus part 1",
        &left,
        0,
        48_000,
        &[(0, 2_048), (14_000, 18_000), (31_553, 33_601)],
    );
    assert!(worst < 0.25, "{worst}");
    let info = &started_info(&render.events)[0];
    println!("opus part 1: {info:?}");
    assert_eq!(
        (info.codec.as_str(), info.gapless, info.sample_rate),
        ("opus", true, 48_000)
    );
    assert_eq!(info.duration_us, Some(700_020));
    let bitrate = info.bitrate.expect("bit rate");
    assert!((40_000..160_000).contains(&bitrate), "{bitrate}");
}

#[test]
fn opus_parts_join_gaplessly() {
    let dir = tempfile::tempdir().unwrap();
    let items = vec![
        item(1, &fixture("opus-part1.opus")),
        item(2, &fixture("opus-part2.opus")),
    ];
    let render = render(dir.path(), items, None, |_| {});

    assert_eq!(render.frames(), 33_601 + 20_000);
    let left = render.left();
    let join = 33_601;
    let worst = check_alignment(
        "opus join",
        &left,
        0,
        48_000,
        &[
            (join - 2_048, join),
            (join, join + 2_048),
            (left.len() - 2_048, left.len()),
        ],
    );
    let bound = sine_step_bound(1_000.0, 48_000, 0.5) * LOSSY_STEP_ALLOWANCE;
    let (step, at) = max_step(&left[join - 64..join + 64]);
    println!(
        "opus join: largest step {step:.4} at {} (sine step x{LOSSY_STEP_ALLOWANCE} {bound:.4})",
        join - 64 + at
    );
    assert!(step < bound, "{step}");
    assert!(worst < 0.25, "{worst}");
}

#[test]
fn opus_output_gain_is_applied() {
    let dir = tempfile::tempdir().unwrap();
    let plain = render(
        dir.path(),
        vec![item(1, &fixture("opus-part1.opus"))],
        None,
        |_| {},
    );
    let sub = dir.path().join("gain");
    std::fs::create_dir(&sub).unwrap();
    let gained = render(&sub, vec![item(1, &fixture("opus-gain.opus"))], None, |_| {});

    assert_eq!(gained.frames(), plain.frames());
    let ratio = rms(&gained.left()[4_800..28_800]) / rms(&plain.left()[4_800..28_800]);
    println!("opus output gain -1536 (Q7.8): measured {:.3} dB", db(ratio));
    assert!((db(ratio) + 6.0).abs() < 0.05, "{ratio}");
}

#[test]
fn opus_seeks_land_on_the_sine() {
    let dir = tempfile::tempdir().unwrap();
    let render = render(
        dir.path(),
        vec![item(1, &fixture("opus-part1.opus"))],
        None,
        |e| e.seek(350),
    );
    let target = 16_800;
    assert_eq!(render.frames(), 33_601 - target);
    let worst = check_alignment("opus seek", &render.left(), target, 48_000, &[(0, 2_048)]);
    assert!(worst < 0.25, "{worst}");
}
