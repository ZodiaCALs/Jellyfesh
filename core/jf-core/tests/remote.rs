//! Remote sessions through the core facade (ADR-0018): playing a remote
//! source's tracks starts a session, the transport commands control it, the
//! `Remote` slice follows, and local playback, `Stop` or `End` end it. The
//! service is the `remote` test plugin: two invented devices and a player
//! that exists only in the plugin's memory.

mod common;

use std::time::Duration;

use common::*;
use jf_core::{
    Command, Event, RemoteAction, RemoteSnapshot, RepeatMode, SearchScope, SourceItem, Status, TrackRef,
};

const REMOTE: &str = "org.jellyfesh.test.remote";

fn remote_manifest() -> String {
    format!(
        "id = {REMOTE:?}\nname = \"Test Remote\"\nversion = \"1.0.0\"\napi_version = \"1.1\"\n\
         author = \"Jellyfesh tests\"\ncapabilities = [\"search\", \"remote\"]\nplayback_mode = \"remote\"\n\
         \n[settings]\nschema = \"settings.schema.json\"\n\
         \n[links]\ntrack = \"https://music.example.com/song/{{id}}\"\n"
    )
}

/// Waits for a `Remote` slice that `ok` accepts.
fn remote_until(env: &Env, command: Command, ok: impl Fn(&RemoteSnapshot) -> bool) -> RemoteSnapshot {
    let events = env.run(command, |e| matches!(e, Event::Remote(r) if ok(r)));
    match events.last() {
        Some(Event::Remote(r)) => (**r).clone(),
        _ => unreachable!(),
    }
}

fn search(env: &Env) -> Vec<SourceItem> {
    let events = env.run(
        Command::Search {
            query: "Sample".into(),
            scope: SearchScope::Plugin { id: REMOTE.into() },
        },
        |e| matches!(e, Event::SearchResults(u) if u.source == REMOTE),
    );
    match events.last() {
        Some(Event::SearchResults(update)) => update.items.clone(),
        _ => unreachable!(),
    }
}

#[test]
fn a_remote_source_plays_on_its_own_devices() {
    let env = Env::new();
    let summary = env.install("remote", &remote_manifest(), REMOTE, true, None);
    assert!(summary.remote_playback);
    assert!(summary.vendor_playback, "no EQ or crossfade either");
    assert!(summary.capabilities.contains(&"remote".to_string()));
    assert_eq!(env.core.snapshot().remote.source, None);

    let items = search(&env);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].link.as_deref(), Some("https://music.example.com/song/a"));
    let tracks: Vec<TrackRef> = items.iter().map(|i| i.track.clone().expect("playable")).collect();

    // Playing starts a session on the active (here: first) device.
    let slice = remote_until(
        &env,
        Command::PlayTracks {
            tracks: tracks.clone(),
            start: 1,
        },
        |r| r.status == Status::Playing && r.device.is_some(),
    );
    assert_eq!(slice.source.as_deref(), Some(REMOTE));
    assert_eq!(slice.source_name.as_deref(), Some("Test Remote"));
    let item = slice.item.clone().expect("a track");
    assert_eq!(item.title, "Sample B");
    assert_eq!(item.link.as_deref(), Some("https://music.example.com/song/b"));
    assert_eq!(
        slice.device.as_ref().map(|d| d.name.as_str()),
        Some("Desk Speaker")
    );
    let names: Vec<_> = slice.devices.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(
        names,
        ["Desk Speaker", "Phone"],
        "the nameless device is left out"
    );
    assert_eq!(
        env.core.snapshot().playback.status,
        Status::Stopped,
        "the engine stays idle"
    );

    // The transport commands go to the service.
    let slice = remote_until(&env, Command::Pause, |r| r.status == Status::Paused);
    assert_eq!(slice.anchor.rate, 0.0);
    remote_until(
        &env,
        Command::SetRepeat {
            mode: RepeatMode::One,
        },
        |r| r.repeat == RepeatMode::One,
    );
    remote_until(&env, Command::SetShuffle { on: true }, |r| r.shuffle);
    remote_until(
        &env,
        Command::Remote {
            action: RemoteAction::SetVolume { percent: 150 },
        },
        |r| r.device.as_ref().and_then(|d| d.volume_percent) == Some(100),
    );
    remote_until(&env, Command::Previous, |r| {
        r.item.as_ref().is_some_and(|i| i.title == "Sample A")
    });
    let slice = remote_until(
        &env,
        Command::Remote {
            action: RemoteAction::Transfer {
                device: "phone".into(),
            },
        },
        |r| r.device.as_ref().is_some_and(|d| d.name == "Phone") && r.status == Status::Playing,
    );
    assert!(slice.devices.iter().any(|d| d.name == "Phone" && d.active));

    // Its songs never join the queue.
    let error = env.fails(Command::Enqueue {
        tracks: tracks.clone(),
        play_next: false,
    });
    assert!(error.contains("plays on your own devices"), "{error}");
    assert_eq!(env.core.snapshot().queue.len, 0);

    // End leaves the service playing and clears the slice.
    let slice = remote_until(
        &env,
        Command::Remote {
            action: RemoteAction::End,
        },
        |r| r.source.is_none(),
    );
    assert_eq!(slice.status, Status::Stopped);
    assert!(
        env.fails(Command::Remote {
            action: RemoteAction::Refresh
        })
        .contains("nothing is playing")
    );
}

#[test]
fn local_playback_and_stop_end_the_session() {
    let env = Env::new();
    env.install("remote", &remote_manifest(), REMOTE, true, None);
    let tracks: Vec<TrackRef> = search(&env).iter().filter_map(|i| i.track.clone()).collect();
    remote_until(
        &env,
        Command::PlayTracks {
            tracks: tracks.clone(),
            start: 0,
        },
        |r| r.status == Status::Playing,
    );

    // Playing a file here ends the session.
    let song = env.dir.path().join("sample.wav");
    sine_wav(&song, 440.0, 0.5);
    let events = env.run(
        Command::PlayTracks {
            tracks: vec![TrackRef::File(song)],
            start: 0,
        },
        |e| matches!(e, Event::Remote(r) if r.source.is_none()),
    );
    assert!(events.iter().any(|e| matches!(e, Event::Remote(_))));
    assert_eq!(env.core.snapshot().remote.source, None);

    // A new session; Stop pauses the service and ends it.
    remote_until(&env, Command::PlayTracks { tracks, start: 1 }, |r| {
        r.status == Status::Playing && r.source.is_some()
    });
    remote_until(&env, Command::Stop, |r| r.source.is_none());
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(env.core.snapshot().remote.source, None, "nothing brings it back");
}
