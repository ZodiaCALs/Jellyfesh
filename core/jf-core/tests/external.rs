//! Players an app provides (ADR-0018 item 5): registered under a source id,
//! they take remote sessions exactly like a remote plugin. The player here is
//! a fake with two invented tracks and one device.

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jf_core::{
    Command, Event, ExternalItem, ExternalPlayer, ExternalState, ItemKind, RemoteAction, RemoteDevice,
    RemoteDeviceKind, RemoteSnapshot, RepeatMode, SourceTrack, Status, TrackRef,
};

#[derive(Default)]
struct Fake {
    state: Mutex<Option<ExternalState>>,
    calls: Mutex<Vec<String>>,
}

impl Fake {
    fn log(&self, call: &str) {
        self.calls.lock().unwrap().push(call.to_owned());
    }
    fn with(&self, f: impl FnOnce(&mut ExternalState)) -> Result<(), String> {
        match self.state.lock().unwrap().as_mut() {
            Some(state) => {
                f(state);
                Ok(())
            }
            None => Err("Nothing is playing in Sample Music.".into()),
        }
    }
}

fn this_mac() -> RemoteDevice {
    RemoteDevice {
        id: "this".into(),
        name: "This Computer".into(),
        kind: RemoteDeviceKind::Computer,
        active: true,
        volume_percent: None,
    }
}

impl ExternalPlayer for Fake {
    fn devices(&self) -> Result<Vec<RemoteDevice>, String> {
        Ok(vec![this_mac()])
    }
    fn state(&self) -> Result<Option<ExternalState>, String> {
        Ok(self.state.lock().unwrap().clone())
    }
    fn play(
        &self,
        context: Option<ExternalItem>,
        tracks: Vec<ExternalItem>,
        start: u32,
    ) -> Result<(), String> {
        self.log(&format!("play {} from {start}", tracks.len()));
        *self.state.lock().unwrap() = Some(ExternalState {
            item: tracks.get(start as usize).cloned(),
            context,
            playing: true,
            position_ms: 0,
            shuffle: false,
            repeat: RepeatMode::Off,
            device: Some(this_mac()),
        });
        Ok(())
    }
    fn resume(&self) -> Result<(), String> {
        self.log("resume");
        self.with(|s| s.playing = true)
    }
    fn pause(&self) -> Result<(), String> {
        self.log("pause");
        self.with(|s| s.playing = false)
    }
    fn next(&self) -> Result<(), String> {
        self.log("next");
        Ok(())
    }
    fn previous(&self) -> Result<(), String> {
        self.log("previous");
        Ok(())
    }
    fn seek(&self, position_ms: u64) -> Result<(), String> {
        self.log(&format!("seek {position_ms}"));
        self.with(|s| s.position_ms = position_ms)
    }
    fn set_volume(&self, _percent: u32) -> Result<(), String> {
        Err("This player has no volume control.".into())
    }
    fn set_shuffle(&self, on: bool) -> Result<(), String> {
        self.with(|s| s.shuffle = on)
    }
    fn set_repeat(&self, mode: RepeatMode) -> Result<(), String> {
        self.with(|s| s.repeat = mode)
    }
    fn transfer(&self, _device: String) -> Result<(), String> {
        Ok(())
    }
}

fn track(id: &str, title: &str) -> TrackRef {
    TrackRef::Source(SourceTrack {
        plugin: "sample-music".into(),
        id: id.into(),
        title: title.into(),
        artist: Some("Sample Artist".into()),
        album: None,
        duration_ms: Some(180_000),
        artwork_url: None,
        isrc: None,
        artist_id: None,
        album_id: None,
    })
}

fn remote_until(env: &Env, command: Command, ok: impl Fn(&RemoteSnapshot) -> bool) -> RemoteSnapshot {
    let events = env.run(command, |e| matches!(e, Event::Remote(r) if ok(r)));
    match events.last() {
        Some(Event::Remote(r)) => (**r).clone(),
        _ => unreachable!(),
    }
}

#[test]
fn an_apps_player_takes_remote_sessions() {
    let env = Env::new();
    let fake = Arc::new(Fake::default());
    assert!(
        env.core
            .register_external_player("org.example", "Nope", fake.clone())
            .is_err(),
        "ids with dots belong to plugins"
    );
    env.core
        .register_external_player("sample-music", "Sample Music", fake.clone())
        .unwrap();

    let tracks = vec![track("s1", "Sample One"), track("s2", "Sample Two")];
    let slice = remote_until(
        &env,
        Command::PlayTracks {
            tracks: tracks.clone(),
            start: 1,
        },
        // The device arrives with the first report after the command.
        |r| r.status == Status::Playing && r.item.is_some() && r.device.is_some(),
    );
    assert_eq!(slice.source.as_deref(), Some("sample-music"));
    assert_eq!(slice.source_name.as_deref(), Some("Sample Music"));
    let item = slice.item.unwrap();
    assert_eq!((item.title.as_str(), item.kind), ("Sample Two", ItemKind::Track));
    assert_eq!(slice.device.map(|d| d.name), Some("This Computer".into()));

    remote_until(&env, Command::Pause, |r| r.status == Status::Paused);
    remote_until(&env, Command::Seek { position_ms: 60_000 }, |r| {
        r.anchor.position_us == 60_000_000
    });
    let slice = remote_until(
        &env,
        Command::Remote {
            action: RemoteAction::SetVolume { percent: 50 },
        },
        |r| r.error.is_some(),
    );
    assert_eq!(slice.error.as_deref(), Some("This player has no volume control."));
    assert!(
        env.fails(Command::Enqueue {
            tracks,
            play_next: true
        })
        .contains("Sample Music plays on your own devices")
    );
    remote_until(
        &env,
        Command::Remote {
            action: RemoteAction::End,
        },
        |r| r.source.is_none(),
    );
    let calls = fake.calls.lock().unwrap().clone();
    assert_eq!(calls[..3], ["play 2 from 1", "pause", "seek 60000"]);

    // Once unregistered, its tracks no longer start a session: the queue
    // takes them and cannot play them.
    env.core.unregister_external_player("sample-music");
    let events = env.run(
        Command::PlayTracks {
            tracks: vec![track("s1", "Sample One")],
            start: 0,
        },
        |e| matches!(e, Event::Error { message } if message.contains("is not installed")),
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Remote(r) if r.source.is_some()))
    );
}
