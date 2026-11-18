//! `JellyfeshCore` as Swift and Kotlin see it (the UniFFI-exported Rust API): the shared
//! scenario, and the listener's guarantees.

mod common;

use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use jf_ffi::*;

/// Forwards every event to a channel.
struct Recorder(Mutex<Sender<Event>>);

impl CoreListener for Recorder {
    fn on_event(&self, event: Event) {
        let _ = self.0.lock().unwrap().send(event);
    }
}

fn recorder() -> (Arc<dyn CoreListener>, Receiver<Event>) {
    let (tx, rx) = mpsc::channel();
    (Arc::new(Recorder(Mutex::new(tx))), rx)
}

fn config(dir: &Path, output: OutputKind) -> CoreConfig {
    CoreConfig {
        data_dir: dir.join("data").to_string_lossy().into_owned(),
        cache_dir: dir.join("cache").to_string_lossy().into_owned(),
        output,
        buffer_ms: 200,
        restore_session: false,
        persist_session: false,
        watch_folders: false,
        bundled_plugins: Vec::new(),
        distribution: Distribution::Source,
    }
}

/// Events until `done` accepts one, or panics after `timeout`.
fn wait_for(rx: &Receiver<Event>, timeout: Duration, mut done: impl FnMut(&Event) -> bool) -> Vec<Event> {
    let end = Instant::now() + timeout;
    let mut seen = Vec::new();
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(event) => {
                let stop = done(&event);
                seen.push(event);
                if stop {
                    return seen;
                }
            }
            Err(_) => break,
        }
    }
    panic!("timed out; events seen: {seen:#?}");
}

/// Events until none has arrived for `quiet`.
fn drain(rx: &Receiver<Event>, quiet: Duration) -> Vec<Event> {
    std::iter::from_fn(|| rx.recv_timeout(quiet).ok()).collect()
}

/// Calls `probe` every `every` until it returns `Some`, or panics after `timeout`.
fn poll<T>(timeout: Duration, every: Duration, what: &str, mut probe: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + timeout;
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(every);
    }
}

#[test]
fn the_shared_scenario() {
    let scenario = common::scenario();
    let expect = &scenario["expect"];
    let timeout = Duration::from_millis(scenario["timeout_ms"].as_u64().unwrap());
    let quiet = Duration::from_millis(scenario["quiet_ms"].as_u64().unwrap());
    let every = Duration::from_millis(scenario["poll_ms"].as_u64().unwrap());
    let dir = tempfile::tempdir().unwrap();
    let music = common::write_tones(dir.path(), &scenario);

    // 1. Start with the scenario's config and temporary folders.
    let mut config_json = scenario["config"].clone();
    config_json["data_dir"] = dir.path().join("data").to_string_lossy().into();
    config_json["cache_dir"] = dir.path().join("cache").to_string_lossy().into();
    let config: CoreConfig = serde_json::from_value(config_json).unwrap();
    let core = JellyfeshCore::new(config).unwrap();
    let (listener, rx) = recorder();
    core.set_listener(Some(listener));

    // 2. AddRoot scans the folder.
    assert!(
        core.send(Command::AddRoot {
            path: music.to_string_lossy().into_owned()
        }) > 0
    );
    let events = wait_for(&rx, timeout, |e| {
        matches!(
            e,
            Event::Scan {
                status: ScanStatus::Finished { .. }
            }
        )
    });
    let Some(Event::Scan {
        status: ScanStatus::Finished { report },
    }) = events.last()
    else {
        unreachable!()
    };
    assert_eq!(report.added, expect["scan_added"].as_u64().unwrap());

    // 3. Query and search the library.
    let tracks = core
        .tracks(TrackQuery {
            offset: 0,
            limit: 100,
            sort: TrackSort::Path,
            descending: false,
            album_id: None,
            artist_id: None,
        })
        .unwrap();
    let found = core
        .search_library(
            scenario["search"]["text"].as_str().unwrap().into(),
            scenario["search"]["limit"].as_u64().unwrap() as u32,
        )
        .unwrap();
    let titles: Vec<String> = found.tracks.iter().map(|t| t.title.clone()).collect();
    assert_eq!(titles, common::strings(&scenario, "/search/titles"));
    assert_eq!(core.roots().unwrap(), vec![music.to_string_lossy().into_owned()]);
    let refs: Vec<TrackRef> = tracks.iter().map(|t| TrackRef::Library { id: t.id }).collect();

    // 4. A playlist of every track.
    let name = scenario["playlist"]["name"].as_str().unwrap().to_owned();
    let request = core.send(Command::CreatePlaylist {
        name: name.clone(),
        tracks: refs.clone(),
    });
    let created = wait_for(
        &rx,
        timeout,
        |e| matches!(e, Event::PlaylistCreated { request: r, .. } if *r == request),
    );
    let Some(&Event::PlaylistCreated { id: playlist, .. }) = created.last() else {
        unreachable!()
    };
    let summary = core.playlist(playlist).unwrap().expect("the new playlist");
    assert_eq!((summary.name.as_str(), summary.entry_count), (name.as_str(), 2));
    let entries = core.playlist_entries(playlist, 0, 50).unwrap();
    let titles: Vec<String> = entries.iter().map(|e| e.row.title.clone()).collect();
    assert_eq!(titles, common::strings(&scenario, "/expect/playlist_titles"));
    assert!(core.playlists().unwrap().iter().any(|p| p.id == playlist));

    // 5. A favourite.
    let favorite = scenario["favorite"].as_str().unwrap();
    let track = refs[tracks.iter().position(|t| t.title == favorite).unwrap()].clone();
    core.send(Command::SetFavorite {
        track: track.clone(),
        favorite: true,
    });
    poll(timeout, every, "the favourite", || {
        core.is_favorite(track.clone()).unwrap().then_some(())
    });
    let titles: Vec<String> = core
        .favorites(0, 50)
        .unwrap()
        .into_iter()
        .map(|r| r.title)
        .collect();
    assert_eq!(titles, common::strings(&scenario, "/expect/favorite_titles"));

    // 6. Export.
    let export = dir
        .path()
        .join(scenario["playlist"]["export_file"].as_str().unwrap());
    core.export_playlist(playlist, export.to_string_lossy().into_owned())
        .unwrap();
    let text = std::fs::read_to_string(&export).unwrap();
    for needle in common::strings(&scenario, "/playlist/export_contains") {
        assert!(text.contains(&needle), "{needle:?} missing from {text}");
    }

    // 7. Play the playlist.
    let id = core.send(Command::PlayCollection {
        collection: Collection::Playlist { id: playlist },
        start: 0,
        shuffle: false,
    });
    assert!(id > 0);
    let mut events = wait_for(
        &rx,
        timeout,
        |e| matches!(e, Event::Playback { snapshot } if snapshot.status == Status::Ended),
    );
    events.extend(drain(&rx, quiet));

    let mut announced: Vec<(u64, String)> = events
        .iter()
        .filter_map(|e| match e {
            Event::NowPlaying { snapshot: Some(np) } => Some((np.uid, np.title.clone())),
            _ => None,
        })
        .collect();
    // Consecutive announcements of the same entry count once (see the scenario's "about").
    announced.dedup_by_key(|(uid, _)| *uid);
    let titles: Vec<String> = announced.into_iter().map(|(_, title)| title).collect();
    assert_eq!(titles, common::strings(&scenario, "/expect/now_playing_titles"));
    let last_playback = events.iter().rev().find_map(|e| match e {
        Event::Playback { snapshot } => Some(snapshot.clone()),
        _ => None,
    });
    let last_queue = events.iter().rev().find_map(|e| match e {
        Event::Queue { snapshot } => Some(snapshot.clone()),
        _ => None,
    });
    let last_now_playing = events.iter().rev().find_map(|e| match e {
        Event::NowPlaying { snapshot } => Some(snapshot.clone()),
        _ => None,
    });
    let snapshot = core.snapshot();
    assert_eq!(snapshot.playback.status, Status::Ended);
    assert_eq!(snapshot.queue.len, expect["queue_len"].as_u64().unwrap() as u32);
    assert_eq!(last_playback.as_ref(), Some(&snapshot.playback));
    assert_eq!(last_queue.as_ref(), Some(&snapshot.queue));
    assert_eq!(last_now_playing, Some(snapshot.now_playing.clone()));
    assert_eq!(core.queue_window(0, 10).unwrap().len(), 2);

    // 8. Play statistics.
    let played = common::strings(&scenario, "/expect/played_titles");
    poll(timeout, every, "the plays", || {
        let mut titles: Vec<String> = core
            .recently_played(50)
            .unwrap()
            .into_iter()
            .map(|r| r.title)
            .collect();
        titles.sort();
        titles.dedup();
        (titles == played).then_some(())
    });

    // 9. Sorted albums and thumbnails.
    assert_eq!(core.albums_sorted(AlbumSort::Title, false, 0, 50), Ok(vec![]));
    let unknown = scenario["unknown_artwork_hash"].as_str().unwrap().to_owned();
    assert_eq!(core.artwork_thumbnail(unknown, 128), Ok(None));

    // 10. Shutdown.
    core.shutdown();
    drain(&rx, Duration::from_millis(50));
    assert_eq!(core.send(Command::Play), 0, "send after shutdown");
    assert_eq!(core.albums(0, 10), Err(CoreFfiError::ShutDown));
    assert_eq!(core.snapshot(), snapshot, "the last state stays readable");
    assert!(
        rx.recv_timeout(Duration::from_millis(200)).is_err(),
        "no event after shutdown"
    );
    core.shutdown();
}

#[test]
fn replacing_the_listener_waits_for_a_call_in_progress() {
    /// Sleeps in its first call and records when that call returned.
    struct Slow(Mutex<Option<Instant>>, Mutex<Sender<()>>);
    impl CoreListener for Slow {
        fn on_event(&self, _event: Event) {
            let mut finished = self.0.lock().unwrap();
            if finished.is_none() {
                let _ = self.1.lock().unwrap().send(());
                std::thread::sleep(Duration::from_millis(300));
                *finished = Some(Instant::now());
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let core = JellyfeshCore::new(config(dir.path(), OutputKind::Null { realtime: false })).unwrap();
    let (started_tx, started) = mpsc::channel();
    let slow = Arc::new(Slow(Mutex::new(None), Mutex::new(started_tx)));
    core.set_listener(Some(slow.clone()));
    core.send(Command::SetVolume { volume: 0.5 });
    started.recv_timeout(Duration::from_secs(10)).unwrap();
    core.set_listener(None);
    let replaced = Instant::now();
    let finished = slow.0.lock().unwrap().expect("the call finished");
    assert!(
        replaced >= finished,
        "set_listener returned while the old listener ran"
    );
}

#[test]
fn a_panicking_listener_loses_one_event_not_the_stream() {
    struct Flaky(Mutex<u32>, Mutex<Sender<Event>>);
    impl CoreListener for Flaky {
        fn on_event(&self, event: Event) {
            let mut calls = self.0.lock().unwrap();
            *calls += 1;
            if *calls == 1 {
                drop(calls);
                panic!("listener bug");
            }
            let _ = self.1.lock().unwrap().send(event);
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let core = JellyfeshCore::new(config(dir.path(), OutputKind::Null { realtime: false })).unwrap();
    let (tx, rx) = mpsc::channel();
    core.set_listener(Some(Arc::new(Flaky(Mutex::new(0), Mutex::new(tx)))));
    core.send(Command::SetVolume { volume: 0.5 });
    core.send(Command::SetVolume { volume: 0.25 });
    wait_for(
        &rx,
        Duration::from_secs(10),
        |e| matches!(e, Event::Playback { snapshot } if snapshot.volume == 0.25),
    );
}

#[test]
fn the_listener_may_shut_the_core_down_from_on_event() {
    struct Stopper(Mutex<Option<Arc<JellyfeshCore>>>, Mutex<Sender<()>>);
    impl CoreListener for Stopper {
        fn on_event(&self, _event: Event) {
            if let Some(core) = self.0.lock().unwrap().take() {
                core.shutdown();
                let _ = self.1.lock().unwrap().send(());
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let core = JellyfeshCore::new(config(dir.path(), OutputKind::Null { realtime: false })).unwrap();
    let (tx, done) = mpsc::channel();
    core.set_listener(Some(Arc::new(Stopper(
        Mutex::new(Some(core.clone())),
        Mutex::new(tx),
    ))));
    core.send(Command::SetVolume { volume: 0.5 });
    done.recv_timeout(Duration::from_secs(10))
        .expect("shut down from on_event");
    assert_eq!(core.stats(), Err(CoreFfiError::ShutDown));
}

#[test]
fn errors_and_free_functions() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a file");
    std::fs::write(&file, b"").unwrap();
    let mut bad = config(dir.path(), OutputKind::Null { realtime: false });
    bad.data_dir = file.join("data").to_string_lossy().into_owned();
    assert!(matches!(JellyfeshCore::new(bad), Err(CoreFfiError::Core(_))));

    let core = JellyfeshCore::new(config(dir.path(), OutputKind::Null { realtime: false })).unwrap();
    assert_eq!(core.playlists(), Ok(vec![]));
    assert_eq!(core.playlist(99), Ok(None));
    assert_eq!(core.track(12345), Ok(None));
    assert_eq!(core.stats().unwrap().tracks, 0);

    assert_eq!(library_version(), env!("CARGO_PKG_VERSION"));
    let a = monotonic_now_ns();
    std::thread::sleep(Duration::from_millis(2));
    assert!(monotonic_now_ns() >= a + 1_000_000);
    if std::env::var_os("HOME").is_some() && std::env::var_os("JF_DATA_DIR").is_none() {
        assert!(default_data_dir().is_some_and(|d| d.to_lowercase().contains("jellyfesh")));
        assert!(default_cache_dir().is_some());
    }
}

/// The app's secret store: records the keys it is asked for; every call fails when `locked`.
struct Keystore {
    asked: Mutex<Vec<String>>,
    locked: bool,
}

impl Keystore {
    fn new(locked: bool) -> Arc<Keystore> {
        Arc::new(Keystore {
            asked: Mutex::new(Vec::new()),
            locked,
        })
    }

    fn answer<T>(&self, key: String, value: T) -> Result<T, SecretStoreError> {
        self.asked.lock().unwrap().push(key);
        if self.locked {
            Err(SecretStoreError::Unavailable {
                reason: "the keychain is locked".into(),
            })
        } else {
            Ok(value)
        }
    }
}

impl SecretStore for Keystore {
    fn get(&self, key: String) -> Result<Option<String>, SecretStoreError> {
        self.answer(key, None)
    }

    fn set(&self, key: String, _value: String) -> Result<(), SecretStoreError> {
        self.answer(key, ())
    }

    fn delete(&self, key: String) -> Result<(), SecretStoreError> {
        self.answer(key, ())
    }
}

#[test]
fn the_apps_secret_store_keeps_the_sign_ins() {
    for (locked, storage) in [
        (false, SecretStorage::Keystore),
        (true, SecretStorage::SessionOnly),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let store = Keystore::new(locked);
        let core = JellyfeshCore::with_secret_store(
            config(dir.path(), OutputKind::Null { realtime: false }),
            store.clone(),
        )
        .unwrap();
        // The core checks the store once at start, off its actor, by reading an entry no
        // plugin can name; a store that fails it is left alone for the rest of the run.
        let asked = poll(
            Duration::from_secs(10),
            Duration::from_millis(10),
            "the check",
            || {
                let asked = store.asked.lock().unwrap().clone();
                (!asked.is_empty()).then_some(asked)
            },
        );
        assert_eq!(asked, vec!["jellyfesh/~probe".to_string()]);
        poll(
            Duration::from_secs(10),
            Duration::from_millis(10),
            "the slice",
            || (core.snapshot().sources.secret_storage == storage).then_some(()),
        );
        assert_eq!(core.snapshot().sources.unavailable, None);
        core.shutdown();
    }
}

/// An app's player through the foreign trait: registered under a source id, its tracks start a
/// remote session and the `Remote` event follows it. The player is a fake with one invented track.
#[test]
fn an_external_player_takes_a_remote_session() {
    struct Player(Mutex<Option<ExternalState>>);
    impl ExternalPlayer for Player {
        fn devices(&self) -> Result<Vec<RemoteDevice>, ExternalPlayerError> {
            Ok(vec![RemoteDevice {
                id: "this".into(),
                name: "This Computer".into(),
                kind: RemoteDeviceKind::Computer,
                active: true,
                volume_percent: None,
            }])
        }
        fn state(&self) -> Result<Option<ExternalState>, ExternalPlayerError> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn play(
            &self,
            context: Option<ExternalItem>,
            tracks: Vec<ExternalItem>,
            start: u32,
        ) -> Result<(), ExternalPlayerError> {
            *self.0.lock().unwrap() = Some(ExternalState {
                item: tracks.get(start as usize).cloned(),
                context,
                playing: true,
                position_ms: 0,
                shuffle: false,
                repeat: RepeatMode::Off,
                device: None,
            });
            Ok(())
        }
        fn resume(&self) -> Result<(), ExternalPlayerError> {
            Ok(())
        }
        fn pause(&self) -> Result<(), ExternalPlayerError> {
            match self.0.lock().unwrap().as_mut() {
                Some(state) => {
                    state.playing = false;
                    Ok(())
                }
                None => Err(ExternalPlayerError::Failed {
                    reason: "Nothing is playing.".into(),
                }),
            }
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
            Err(ExternalPlayerError::Failed {
                reason: "No volume here.".into(),
            })
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

    let dir = tempfile::tempdir().unwrap();
    let core = JellyfeshCore::new(config(dir.path(), OutputKind::Null { realtime: false })).unwrap();
    let (tx, rx) = mpsc::channel();
    core.set_listener(Some(Arc::new(Recorder(Mutex::new(tx)))));
    let player = Arc::new(Player(Mutex::new(None)));
    assert!(
        core.register_external_player("a.b".into(), "Dots".into(), player.clone())
            .is_err()
    );
    core.register_external_player("sample-music".into(), "Sample Music".into(), player)
        .unwrap();
    let track = TrackRef::Source {
        track: SourceTrack {
            plugin: "sample-music".into(),
            id: "s1".into(),
            title: "Sample One".into(),
            artist: None,
            album: None,
            duration_ms: Some(120_000),
            artwork_url: None,
            isrc: None,
        },
    };
    core.send(Command::PlayTracks {
        tracks: vec![track],
        start: 0,
    });
    let events = wait_for(
        &rx,
        Duration::from_secs(20),
        |e| matches!(e, Event::Remote { snapshot } if snapshot.status == Status::Playing && snapshot.item.is_some()),
    );
    let Some(Event::Remote { snapshot }) = events.last() else {
        unreachable!()
    };
    assert_eq!(snapshot.source_name.as_deref(), Some("Sample Music"));
    assert_eq!(
        snapshot.item.as_ref().map(|i| i.title.as_str()),
        Some("Sample One")
    );
    core.send(Command::Remote {
        action: RemoteAction::SetVolume { percent: 10 },
    });
    wait_for(
        &rx,
        Duration::from_secs(20),
        |e| matches!(e, Event::Remote { snapshot } if snapshot.error.as_deref() == Some("No volume here.")),
    );
    core.unregister_external_player("sample-music".into());
    core.shutdown();
}
