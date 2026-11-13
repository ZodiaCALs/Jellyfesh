//! MPRIS over D-Bus (mpris-server on zbus): media keys, the desktop's media
//! controls and other players' remotes see and drive Jellyfesh.
//!
//! The server runs on GTK's main loop. Properties are read from the app's
//! mirror of the core when asked; `Position` is computed from the display
//! clock on demand, never pushed (MPRIS clients extrapolate with `Rate` and
//! the `Seeked` signal), so MPRIS adds no timers.

use std::cell::Cell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use jf_core::{Command, RepeatMode, Status, TrackRef};
use mpris_server::zbus::{self, fdo};
use mpris_server::{
    LocalPlayerInterface, LocalRootInterface, LocalServer, LoopStatus, Metadata, PlaybackRate,
    PlaybackStatus, Property, Signal, Time, TrackId, Volume,
};

use crate::ui::APP_ID;
use crate::ui::state::{Change, app, try_app};
use crate::ui::window;

/// Bus name suffix: `org.mpris.MediaPlayer2.Jellyfesh`.
const BUS_SUFFIX: &str = "Jellyfesh";

struct Mpris;

fn track_id(uid: Option<u64>) -> TrackId {
    uid.and_then(|u| TrackId::try_from(format!("/org/jellyfesh/Jellyfesh/track/{u}")).ok())
        .unwrap_or(TrackId::NO_TRACK)
}

fn metadata() -> Metadata {
    let Some(a) = try_app() else {
        return Metadata::new();
    };
    let now = a.now.borrow().clone();
    let Some(np) = now else { return Metadata::new() };
    let mut b = Metadata::builder()
        .trackid(track_id(Some(np.uid)))
        .title(np.title.clone());
    if let Some(artist) = &np.artist {
        b = b.artist([artist.clone()]);
    }
    if let Some(album) = &np.album {
        b = b.album(album.clone());
    }
    if let Some(us) = a.duration_us() {
        b = b.length(Time::from_micros(us));
    }
    if let Some(jf_core::ArtworkRef::File(path)) = &np.artwork {
        b = b.art_url(gtk::gio::File::for_path(path).uri().to_string());
    }
    if let Some(path) = &np.path {
        b = b.url(gtk::gio::File::for_path(path).uri().to_string());
    }
    b.build()
}

fn status() -> PlaybackStatus {
    match try_app().map(|a| a.status()) {
        Some(Status::Playing) => PlaybackStatus::Playing,
        Some(Status::Paused) => PlaybackStatus::Paused,
        _ => PlaybackStatus::Stopped,
    }
}

fn loop_status() -> LoopStatus {
    match try_app().map(|a| a.queue.borrow().repeat) {
        Some(RepeatMode::All) => LoopStatus::Playlist,
        Some(RepeatMode::One) => LoopStatus::Track,
        _ => LoopStatus::None,
    }
}

fn has_current() -> bool {
    try_app().is_some_and(|a| a.queue.borrow().current.is_some())
}

fn send(command: Command) {
    if let Some(a) = try_app() {
        a.send(command);
    }
}

impl LocalRootInterface for Mpris {
    async fn raise(&self) -> fdo::Result<()> {
        if let Some(s) = window::shell() {
            s.window.present();
        }
        Ok(())
    }

    async fn quit(&self) -> fdo::Result<()> {
        if let Some(app) = gtk::gio::Application::default() {
            app.quit();
        }
        Ok(())
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
        Ok(())
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok("Jellyfesh".into())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok(APP_ID.into())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["file".into()])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok([
            "audio/flac",
            "audio/mpeg",
            "audio/mp4",
            "audio/ogg",
            "audio/x-wav",
            "audio/aiff",
        ]
        .map(String::from)
        .to_vec())
    }
}

impl LocalPlayerInterface for Mpris {
    async fn next(&self) -> fdo::Result<()> {
        send(Command::Next);
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        send(Command::Previous);
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        send(Command::Pause);
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        send(Command::TogglePlayPause);
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        send(Command::Stop);
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        send(Command::Play);
        Ok(())
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        if let Some(a) = try_app() {
            let pos = a.position_us() + offset.as_micros();
            let max = a.duration_us().unwrap_or(i64::MAX);
            let ms = pos.clamp(0, max) / 1_000;
            a.send(Command::Seek {
                position_ms: u64::try_from(ms).unwrap_or(0),
            });
        }
        Ok(())
    }

    async fn set_position(&self, track: TrackId, position: Time) -> fdo::Result<()> {
        let current = try_app().and_then(|a| a.now.borrow().as_ref().map(|n| n.uid));
        if track == track_id(current) && position.as_micros() >= 0 {
            send(Command::Seek {
                position_ms: u64::try_from(position.as_millis()).unwrap_or(0),
            });
        }
        Ok(())
    }

    async fn open_uri(&self, uri: String) -> fdo::Result<()> {
        let file = gtk::gio::File::for_uri(&uri);
        match file.path() {
            Some(path) => {
                send(Command::PlayTracks {
                    tracks: vec![TrackRef::File(path)],
                    start: 0,
                });
                Ok(())
            }
            None => Err(fdo::Error::NotSupported("only local files can be opened".into())),
        }
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        Ok(status())
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(loop_status())
    }

    async fn set_loop_status(&self, loop_status: LoopStatus) -> zbus::Result<()> {
        let mode = match loop_status {
            LoopStatus::None => RepeatMode::Off,
            LoopStatus::Track => RepeatMode::One,
            LoopStatus::Playlist => RepeatMode::All,
        };
        send(Command::SetRepeat { mode });
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn set_rate(&self, _rate: PlaybackRate) -> zbus::Result<()> {
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(try_app().is_some_and(|a| a.queue.borrow().shuffle))
    }

    async fn set_shuffle(&self, shuffle: bool) -> zbus::Result<()> {
        send(Command::SetShuffle { on: shuffle });
        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(metadata())
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(try_app().map_or(1.0, |a| f64::from(a.playback.borrow().volume)))
    }

    async fn set_volume(&self, volume: Volume) -> zbus::Result<()> {
        send(Command::SetVolume {
            volume: volume.clamp(0.0, 1.0) as f32,
        });
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(Time::from_micros(try_app().map_or(0, |a| a.position_us())))
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(has_current())
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(has_current())
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(has_current())
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(has_current())
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(try_app().is_some_and(|a| a.duration_us().is_some()))
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

/// Registers on the session bus; does nothing without one.
pub fn start() {
    glib::spawn_future_local(async {
        let server = match LocalServer::new(BUS_SUFFIX, Mpris).await {
            Ok(s) => Rc::new(s),
            Err(e) => {
                tracing::info!("MPRIS is not available: {e}");
                return;
            }
        };
        glib::spawn_future_local(server.run());
        let last_generation = Rc::new(Cell::new(None::<(u64, u64)>));
        // The listener owns the server: dropping it would release the bus
        // name. It lives as long as the app state.
        app().listen(move |change| {
            let props: Vec<Property> = match change {
                Change::Playback => {
                    // A seek within the same track: tell clients the jump.
                    let (generation, uid) = {
                        let a = app();
                        let generation = a.playback.borrow().anchor.track_generation;
                        let uid = a.now.borrow().as_ref().map_or(0, |n| n.uid);
                        (generation, uid)
                    };
                    let seeked = last_generation
                        .replace(Some((generation, uid)))
                        .is_some_and(|(g, u)| g != generation && u == uid);
                    if seeked {
                        let s = server.clone();
                        glib::spawn_future_local(async move {
                            let position = Time::from_micros(app().position_us());
                            let _ = s.emit(Signal::Seeked { position }).await;
                        });
                    }
                    vec![
                        Property::PlaybackStatus(status()),
                        Property::Volume(f64::from(app().playback.borrow().volume)),
                        Property::CanSeek(app().duration_us().is_some()),
                    ]
                }
                Change::NowPlaying => vec![
                    Property::Metadata(metadata()),
                    Property::CanGoNext(has_current()),
                    Property::CanGoPrevious(has_current()),
                    Property::CanPlay(has_current()),
                    Property::CanPause(has_current()),
                ],
                Change::Queue => vec![
                    Property::LoopStatus(loop_status()),
                    Property::Shuffle(app().queue.borrow().shuffle),
                ],
                _ => return,
            };
            let s = server.clone();
            glib::spawn_future_local(async move {
                if let Err(e) = s.properties_changed(props).await {
                    tracing::debug!("MPRIS update failed: {e}");
                }
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_ids_are_object_paths() {
        assert_eq!(track_id(Some(7)).as_str(), "/org/jellyfesh/Jellyfesh/track/7");
        assert_eq!(track_id(None), TrackId::NO_TRACK);
    }
}
