//! What the core costs while a song plays and while nothing plays: events, the bytes the C ABI
//! carries for them, allocations, and each thread's wake-ups (context switches, from
//! `/proc/self/task`, so Linux only). The phases: idle with a watched library, playing a generated
//! tone through the paced null output, paused, and a remote session with a player of the test's
//! own. It measures and checks nothing, so it is not part of the normal run:
//!
//! `cargo test -p jf-ffi --test cost -- --ignored --nocapture`

// A counting global allocator.
#![allow(unsafe_code)]

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use jf_ffi::*;

struct Counting;

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

// SAFETY: forwards to the system allocator; the counters are atomics.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(layout.size() as u64, Relaxed);
        // SAFETY: the caller's contract.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(layout.size() as u64, Relaxed);
        // SAFETY: the caller's contract.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(new_size as u64, Relaxed);
        // SAFETY: the caller's contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller's contract.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// Every event the listener got: its kind and the size of its JSON (what the C ABI hands C#).
#[derive(Default)]
struct Tally(Mutex<Vec<(String, usize)>>);

impl CoreListener for Tally {
    fn on_event(&self, event: Event) {
        let json = serde_json::to_string(&event).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let kind = value["type"].as_str().unwrap_or("?").to_owned();
        self.0.lock().unwrap().push((kind, json.len()));
    }
}

/// A thread's name, context switches and CPU ticks, from `/proc/self/task` (empty elsewhere).
#[derive(Debug, Clone, Default)]
struct ThreadSample {
    name: String,
    switches: u64,
    ticks: u64,
}

fn threads() -> BTreeMap<u32, ThreadSample> {
    let mut out = BTreeMap::new();
    let Ok(tasks) = std::fs::read_dir("/proc/self/task") else {
        return out;
    };
    for entry in tasks.flatten() {
        let Ok(tid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let dir = entry.path();
        let name = std::fs::read_to_string(dir.join("comm")).unwrap_or_default();
        let status = std::fs::read_to_string(dir.join("status")).unwrap_or_default();
        let switches = status
            .lines()
            .filter(|l| {
                l.starts_with("voluntary_ctxt_switches") || l.starts_with("nonvoluntary_ctxt_switches")
            })
            .filter_map(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
            .sum();
        let stat = std::fs::read_to_string(dir.join("stat")).unwrap_or_default();
        // Fields after the parenthesised name: state is field 3, utime 14, stime 15.
        let after = stat.rsplit_once(')').map(|(_, rest)| rest).unwrap_or("");
        let fields: Vec<&str> = after.split_whitespace().collect();
        let ticks = fields.get(11).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0)
            + fields.get(12).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        out.insert(
            tid,
            ThreadSample {
                name: name.trim().to_owned(),
                switches,
                ticks,
            },
        );
    }
    out
}

struct Window {
    label: &'static str,
    seconds: f64,
    allocs: u64,
    bytes: u64,
    events: BTreeMap<String, (usize, usize)>,
    /// Per thread name: wake-ups per second, CPU ticks, threads.
    wakeups: BTreeMap<String, (f64, u64, usize)>,
}

fn measure(label: &'static str, tally: &Tally, length: Duration) -> Window {
    measure_from(label, tally, length, || {})
}

/// [`measure`], calling `begin` once the window has opened.
fn measure_from(label: &'static str, tally: &Tally, length: Duration, begin: impl FnOnce()) -> Window {
    let before_threads = threads();
    let first_event = tally.0.lock().unwrap().len();
    let allocs = ALLOCS.load(Relaxed);
    let bytes = BYTES.load(Relaxed);
    let start = Instant::now();
    begin();
    std::thread::sleep(length.saturating_sub(start.elapsed()));
    let seconds = start.elapsed().as_secs_f64();
    let allocs = ALLOCS.load(Relaxed) - allocs;
    let bytes = BYTES.load(Relaxed) - bytes;
    let after_threads = threads();
    let mut events = BTreeMap::new();
    for (kind, size) in tally.0.lock().unwrap()[first_event..].iter() {
        let e = events.entry(kind.clone()).or_insert((0, 0));
        e.0 += 1;
        e.1 += size;
    }
    // Thread names in /proc are cut to 15 bytes.
    let me: String = std::thread::current()
        .name()
        .unwrap_or("")
        .chars()
        .take(15)
        .collect();
    let mut wakeups: BTreeMap<String, (f64, u64, usize)> = BTreeMap::new();
    for (tid, after) in &after_threads {
        let before = before_threads.get(tid).cloned().unwrap_or_default();
        let name = if after.name == me || after.name.starts_with("cost-") {
            "(test)".to_owned()
        } else {
            after.name.clone()
        };
        let w = wakeups.entry(name).or_insert((0.0, 0, 0));
        w.0 += (after.switches - before.switches) as f64 / seconds;
        w.1 += after.ticks - before.ticks;
        w.2 += 1;
    }
    Window {
        label,
        seconds,
        allocs,
        bytes,
        events,
        wakeups,
    }
}

fn report(w: &Window) {
    println!("== {} ({:.1} s)", w.label, w.seconds);
    println!(
        "   allocations: {:.0}/s, {:.0} bytes/s",
        w.allocs as f64 / w.seconds,
        w.bytes as f64 / w.seconds
    );
    let total: usize = w.events.values().map(|(n, _)| n).sum();
    let total_bytes: usize = w.events.values().map(|(_, b)| b).sum();
    println!(
        "   events: {total} ({:.2}/s), C ABI JSON {:.0} bytes/s",
        total as f64 / w.seconds,
        total_bytes as f64 / w.seconds
    );
    for (kind, (n, b)) in &w.events {
        println!("     {kind}: {n} events, {b} bytes");
    }
    println!("   threads (wake-ups/s, CPU ticks, count):");
    for (name, (rate, ticks, count)) in &w.wakeups {
        println!("     {name:<24} {rate:>8.1} {ticks:>5} x{count}");
    }
}

fn config(dir: &Path, output: OutputKind) -> CoreConfig {
    CoreConfig {
        data_dir: dir.join("data").to_string_lossy().into_owned(),
        cache_dir: dir.join("cache").to_string_lossy().into_owned(),
        output,
        buffer_ms: 200,
        restore_session: true,
        persist_session: true,
        watch_folders: true,
        resume_on_start: false,
        bundled_plugins: Vec::new(),
        distribution: Distribution::Source,
        downloads_dir: Some(dir.join("downloads").to_string_lossy().into_owned()),
    }
}

fn wait_until(what: &str, timeout: Duration, mut probe: impl FnMut() -> bool) {
    let end = Instant::now() + timeout;
    while !probe() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
#[ignore = "a measurement: run it with --ignored --nocapture"]
fn what_playback_and_idle_cost() {
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    std::fs::create_dir_all(&music).unwrap();
    let song = music.join("tone.wav");
    common::sine_wav(&song, 44_100, 440.0, 40.0);

    let tally = Arc::new(Tally::default());
    let core = JellyfeshCore::new(config(dir.path(), OutputKind::Null { realtime: true })).unwrap();
    core.set_listener(Some(tally.clone()));
    core.send(Command::AddRoot {
        path: music.to_string_lossy().into_owned(),
    });
    std::thread::sleep(Duration::from_secs(3));
    let idle = measure(
        "idle, nothing played yet (library watched)",
        &tally,
        Duration::from_secs(5),
    );

    let startup = measure_from(
        "the first 3 s of playback",
        &tally,
        Duration::from_secs(3),
        || {
            core.send(Command::PlayTracks {
                tracks: vec![TrackRef::File {
                    path: song.to_string_lossy().into_owned(),
                }],
                start: 0,
            });
        },
    );
    assert_eq!(core.snapshot().playback.status, Status::Playing);
    let playing = measure("steady playback", &tally, Duration::from_secs(10));

    core.send(Command::Pause);
    wait_until("pause", Duration::from_secs(10), || {
        core.snapshot().playback.status == Status::Paused
    });
    std::thread::sleep(Duration::from_secs(1));
    let paused = measure("paused", &tally, Duration::from_secs(10));

    // A remote session (ADR-0018) with a player the app provides, as Apple Music's is on macOS;
    // a plugin's (Spotify's) is polled the same way, each poll being a plugin call.
    let service = Arc::new(Service::default());
    core.register_external_player("sample-music".into(), "Sample Music".into(), service.clone())
        .unwrap();
    core.send(Command::PlayTracks {
        tracks: vec![TrackRef::Source {
            track: SourceTrack {
                plugin: "sample-music".into(),
                id: "s1".into(),
                title: "Sample One".into(),
                artist: None,
                album: None,
                duration_ms: Some(600_000),
                artwork_url: None,
                isrc: None,
            },
        }],
        start: 0,
    });
    wait_until("the remote session", Duration::from_secs(20), || {
        core.snapshot().remote.status == Status::Playing
    });
    std::thread::sleep(Duration::from_secs(2));
    let polls = service.polls.load(Relaxed);
    let remote = measure("remote session, playing", &tally, Duration::from_secs(10));
    let remote_polls = service.polls.load(Relaxed) - polls;
    core.send(Command::Pause);
    wait_until("the remote pause", Duration::from_secs(20), || {
        core.snapshot().remote.status == Status::Paused
    });
    std::thread::sleep(Duration::from_secs(2));
    let polls = service.polls.load(Relaxed);
    let remote_paused = measure("remote session, paused", &tally, Duration::from_secs(15));
    let remote_paused_polls = service.polls.load(Relaxed) - polls;

    for w in [&idle, &startup, &playing, &paused, &remote, &remote_paused] {
        report(w);
    }
    println!(
        "remote state asked: {:.2}/s playing, {:.2}/s paused",
        remote_polls as f64 / remote.seconds,
        remote_paused_polls as f64 / remote_paused.seconds
    );
    core.shutdown();
}

/// A service's player that plays in real time and counts how often it is asked for its state.
#[derive(Default)]
struct Service {
    state: Mutex<Option<(ExternalState, Instant)>>,
    polls: AtomicU64,
}

impl ExternalPlayer for Service {
    fn devices(&self) -> Result<Vec<RemoteDevice>, ExternalPlayerError> {
        Ok(Vec::new())
    }
    fn state(&self) -> Result<Option<ExternalState>, ExternalPlayerError> {
        self.polls.fetch_add(1, Relaxed);
        Ok(self.state.lock().unwrap().as_ref().map(|(state, since)| {
            let mut state = state.clone();
            if state.playing {
                state.position_ms += since.elapsed().as_millis() as u64;
            }
            state
        }))
    }
    fn play(
        &self,
        context: Option<ExternalItem>,
        tracks: Vec<ExternalItem>,
        start: u32,
    ) -> Result<(), ExternalPlayerError> {
        let state = ExternalState {
            item: tracks.get(start as usize).cloned(),
            context,
            playing: true,
            position_ms: 0,
            shuffle: false,
            repeat: RepeatMode::Off,
            device: None,
        };
        *self.state.lock().unwrap() = Some((state, Instant::now()));
        Ok(())
    }
    fn resume(&self) -> Result<(), ExternalPlayerError> {
        Ok(())
    }
    fn pause(&self) -> Result<(), ExternalPlayerError> {
        if let Some((state, since)) = self.state.lock().unwrap().as_mut() {
            state.position_ms += since.elapsed().as_millis() as u64;
            state.playing = false;
            *since = Instant::now();
        }
        Ok(())
    }
    fn next(&self) -> Result<(), ExternalPlayerError> {
        Ok(())
    }
    fn previous(&self) -> Result<(), ExternalPlayerError> {
        Ok(())
    }
    fn seek(&self, _position_ms: u64) -> Result<(), ExternalPlayerError> {
        Ok(())
    }
    fn set_volume(&self, _percent: u32) -> Result<(), ExternalPlayerError> {
        Ok(())
    }
    fn set_shuffle(&self, _on: bool) -> Result<(), ExternalPlayerError> {
        Ok(())
    }
    fn set_repeat(&self, _mode: RepeatMode) -> Result<(), ExternalPlayerError> {
        Ok(())
    }
    fn transfer(&self, _device: String) -> Result<(), ExternalPlayerError> {
        Ok(())
    }
}
