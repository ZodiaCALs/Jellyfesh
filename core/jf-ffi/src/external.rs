//! Players the apps provide (ADR-0018 item 5) as a foreign trait: the macOS app implements
//! [`ExternalPlayer`] over MusicKit's `ApplicationMusicPlayer` and registers it with
//! `JellyfeshCore::register_external_player`. The core drives it like a remote source. The C#
//! shell has no external player, so the C ABI does not carry one.

use std::sync::Arc;

use jf_core as jf;

use crate::convert::Conv;
use crate::types::{ItemKind, RemoteDevice, RepeatMode};

/// An item an [`ExternalPlayer`] plays or reports.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ExternalItem {
    pub kind: ItemKind,
    /// The player's own id for it.
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub artwork_url: Option<String>,
}

/// What an [`ExternalPlayer`] is doing.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ExternalState {
    pub item: Option<ExternalItem>,
    /// The album or playlist it plays from, when it knows.
    pub context: Option<ExternalItem>,
    pub playing: bool,
    pub position_ms: u64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub device: Option<RemoteDevice>,
}

/// A player the app provides, registered under a source id without dots (`apple-music`).
/// Sending `PlayTracks` with `TrackRef::Source` tracks whose `plugin` is that id (or
/// `PlayCollection` with `Collection::Source { plugin: id, .. }`) starts a remote session on it,
/// and the `Remote` slice follows it like any remote source.
///
/// The core calls it from the remote session's own thread, never from the main thread; a call
/// may block until the player answers (keep it to a few seconds). Throw
/// `ExternalPlayerError.Failed` with a sentence the person can read; it shows as the slice's
/// `error`.
#[uniffi::export(with_foreign)]
pub trait ExternalPlayer: Send + Sync {
    fn devices(&self) -> Result<Vec<RemoteDevice>, ExternalPlayerError>;
    /// `None` when nothing plays.
    fn state(&self) -> Result<Option<ExternalState>, ExternalPlayerError>;
    /// Plays `tracks` from `start`, or the album or playlist `context` from its track `start`
    /// (then `tracks` is empty).
    fn play(
        &self,
        context: Option<ExternalItem>,
        tracks: Vec<ExternalItem>,
        start: u32,
    ) -> Result<(), ExternalPlayerError>;
    fn resume(&self) -> Result<(), ExternalPlayerError>;
    fn pause(&self) -> Result<(), ExternalPlayerError>;
    fn next(&self) -> Result<(), ExternalPlayerError>;
    fn previous(&self) -> Result<(), ExternalPlayerError>;
    fn seek(&self, position_ms: u64) -> Result<(), ExternalPlayerError>;
    /// 0–100; a player without a volume throws.
    fn set_volume(&self, percent: u32) -> Result<(), ExternalPlayerError>;
    fn set_shuffle(&self, on: bool) -> Result<(), ExternalPlayerError>;
    fn set_repeat(&self, mode: RepeatMode) -> Result<(), ExternalPlayerError>;
    fn transfer(&self, device: String) -> Result<(), ExternalPlayerError>;
}

/// Why an [`ExternalPlayer`] call failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum ExternalPlayerError {
    /// `reason` is shown to the person. (Not `message`, which Kotlin exceptions already have.)
    #[error("{reason}")]
    Failed { reason: String },
}

impl From<uniffi::UnexpectedUniFFICallbackError> for ExternalPlayerError {
    fn from(error: uniffi::UnexpectedUniFFICallbackError) -> Self {
        ExternalPlayerError::Failed { reason: error.reason }
    }
}

impl From<ExternalItem> for jf::ExternalItem {
    fn from(i: ExternalItem) -> Self {
        jf::ExternalItem {
            kind: i.kind.into(),
            id: i.id,
            title: i.title,
            artist: i.artist,
            album: i.album,
            duration_ms: i.duration_ms,
            artwork_url: i.artwork_url,
        }
    }
}

impl From<jf::ExternalItem> for ExternalItem {
    fn from(i: jf::ExternalItem) -> Self {
        ExternalItem {
            kind: i.kind.into(),
            id: i.id,
            title: i.title,
            artist: i.artist,
            album: i.album,
            duration_ms: i.duration_ms,
            artwork_url: i.artwork_url,
        }
    }
}

impl From<ExternalState> for jf::ExternalState {
    fn from(s: ExternalState) -> Self {
        jf::ExternalState {
            item: s.item.map(Into::into),
            context: s.context.map(Into::into),
            playing: s.playing,
            position_ms: s.position_ms,
            shuffle: s.shuffle,
            repeat: s.repeat.into(),
            device: s.device.conv(),
        }
    }
}

/// The app's player as the core's [`jf::ExternalPlayer`].
pub(crate) struct Foreign(pub(crate) Arc<dyn ExternalPlayer>);

fn text(error: ExternalPlayerError) -> String {
    error.to_string()
}

impl jf::ExternalPlayer for Foreign {
    fn devices(&self) -> Result<Vec<jf::RemoteDevice>, String> {
        Ok(self.0.devices().map_err(text)?.conv())
    }
    fn state(&self) -> Result<Option<jf::ExternalState>, String> {
        Ok(self.0.state().map_err(text)?.map(Into::into))
    }
    fn play(
        &self,
        context: Option<jf::ExternalItem>,
        tracks: Vec<jf::ExternalItem>,
        start: u32,
    ) -> Result<(), String> {
        self.0
            .play(
                context.map(Into::into),
                tracks.into_iter().map(Into::into).collect(),
                start,
            )
            .map_err(text)
    }
    fn resume(&self) -> Result<(), String> {
        self.0.resume().map_err(text)
    }
    fn pause(&self) -> Result<(), String> {
        self.0.pause().map_err(text)
    }
    fn next(&self) -> Result<(), String> {
        self.0.next().map_err(text)
    }
    fn previous(&self) -> Result<(), String> {
        self.0.previous().map_err(text)
    }
    fn seek(&self, position_ms: u64) -> Result<(), String> {
        self.0.seek(position_ms).map_err(text)
    }
    fn set_volume(&self, percent: u32) -> Result<(), String> {
        self.0.set_volume(percent).map_err(text)
    }
    fn set_shuffle(&self, on: bool) -> Result<(), String> {
        self.0.set_shuffle(on).map_err(text)
    }
    fn set_repeat(&self, mode: jf::RepeatMode) -> Result<(), String> {
        self.0.set_repeat(mode.into()).map_err(text)
    }
    fn transfer(&self, device: String) -> Result<(), String> {
        self.0.transfer(device).map_err(text)
    }
}
