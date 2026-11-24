//! Remote playback as the player shows it (ADR-0018; docs/ui.md, "Links and
//! remote playback"): what the player bar, the mini-player and Now Playing
//! show while the core's `Remote` slice has a source, the rows of the
//! Devices list, and the error to show once when a session ends. The core
//! runs the session and decides everything; this only reads its slice.

use jf_core::{ArtworkRef, RemoteDevice, RemoteDeviceKind, RemoteSnapshot, RepeatMode, Status};

/// The remote session's track, as the player shows it instead of the
/// engine's.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteView {
    /// The source in control (its plugin id).
    pub source: String,
    /// Its name: "Open in {Service}", and the Lyrics and Queue panes.
    pub service: String,
    /// The track, when the service reports one.
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub artwork: Option<ArtworkRef>,
    pub duration_ms: Option<u64>,
    /// Opens the track in its service.
    pub link: Option<String>,
    /// The playing device, for "Playing on {Device}".
    pub device: Option<String>,
    pub device_kind: Option<RemoteDeviceKind>,
    /// What went wrong last talking to the service, for a quieter line.
    pub error: Option<String>,
    pub playing: bool,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    /// The playing device's volume (0–100); `None` hides the volume
    /// control.
    pub volume: Option<u32>,
}

/// What to show for the slice; `None` without a remote session, when the
/// engine's track shows.
pub fn view(slice: &RemoteSnapshot) -> Option<RemoteView> {
    let source = slice.source.clone()?;
    let item = slice.item.as_ref();
    let device = slice.device.as_ref();
    Some(RemoteView {
        service: slice
            .source_name
            .clone()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| source.clone()),
        source,
        title: item.map(|i| i.title.clone()).filter(|t| !t.trim().is_empty()),
        artist: item
            .and_then(|i| i.subtitle.clone())
            .filter(|a| !a.trim().is_empty()),
        album: item
            .and_then(|i| i.album.clone())
            .filter(|a| !a.trim().is_empty()),
        artwork: item.and_then(|i| i.artwork.clone()),
        duration_ms: item.and_then(|i| i.duration_ms),
        link: item.and_then(|i| i.link.clone()),
        device: device.map(|d| d.name.clone()).filter(|n| !n.trim().is_empty()),
        device_kind: device.map(|d| d.kind),
        error: slice.error.clone().filter(|e| !e.trim().is_empty()),
        playing: slice.status == Status::Playing,
        shuffle: slice.shuffle,
        repeat: slice.repeat,
        volume: device.and_then(|d| d.volume_percent).map(|v| v.min(100)),
    })
}

/// One row of the Devices list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRow {
    pub id: String,
    pub name: String,
    /// The glyph for its kind.
    pub glyph: &'static str,
    /// The playing device carries the check.
    pub checked: bool,
}

/// The glyph for a kind of device (Misty outline glyphs, `glyph.rs`).
pub fn device_glyph(kind: RemoteDeviceKind) -> &'static str {
    match kind {
        RemoteDeviceKind::Computer => "device-computer",
        RemoteDeviceKind::Smartphone => "device-phone",
        RemoteDeviceKind::Speaker => "device-speaker",
        RemoteDeviceKind::Tv => "device-tv",
        RemoteDeviceKind::Car => "device-car",
        RemoteDeviceKind::Other => "device-other",
    }
}

/// The Devices list: every device the service lists, in its order, the
/// playing one checked. The playing device comes first when the list does
/// not have it yet (the list is asked for when the picker opens).
pub fn device_rows(slice: &RemoteSnapshot) -> Vec<DeviceRow> {
    let playing = slice.device.as_ref();
    let is_playing = |d: &RemoteDevice| match playing {
        Some(p) => p.id == d.id,
        None => d.active,
    };
    let row = |d: &RemoteDevice, checked: bool| DeviceRow {
        id: d.id.clone(),
        name: d.name.clone(),
        glyph: device_glyph(d.kind),
        checked,
    };
    let mut rows: Vec<DeviceRow> = slice
        .devices
        .iter()
        .filter(|d| !d.name.trim().is_empty())
        .map(|d| row(d, is_playing(d)))
        .collect();
    if let Some(p) = playing
        && !p.name.trim().is_empty()
        && !rows.iter().any(|r| r.id == p.id)
    {
        rows.insert(0, row(p, true));
    }
    rows
}

/// The message to show once when a session ends with an error ("Sign in to
/// Spotify again."): the slice had a session and has none now. Errors
/// while a session runs show in its quieter line instead.
pub fn ended_error(before: &RemoteSnapshot, after: &RemoteSnapshot) -> Option<String> {
    if before.source.is_some() && after.source.is_none() {
        after.error.clone().filter(|e| !e.trim().is_empty())
    } else {
        None
    }
}

/// The remote volume for a slider position (0 to 1), in whole percent.
pub fn volume_percent(value: f64) -> u32 {
    (value.clamp(0.0, 1.0) * 100.0).round() as u32
}

/// The remote volume `step` percent louder (or quieter, when negative),
/// within 0–100: Ctrl+↑ and Ctrl+↓ while a session plays.
pub fn volume_step(percent: u32, step: i32) -> u32 {
    (i64::from(percent) + i64::from(step)).clamp(0, 100) as u32
}

/// The repeat mode after the repeat button: off, all, one, off.
pub fn next_repeat(mode: RepeatMode) -> RepeatMode {
    match mode {
        RepeatMode::Off => RepeatMode::All,
        RepeatMode::All => RepeatMode::One,
        RepeatMode::One => RepeatMode::Off,
    }
}

#[cfg(test)]
mod tests {
    use jf_core::{ItemKind, PlaybackAnchor, SourceItem};

    use super::*;

    fn device(
        id: &str,
        name: &str,
        kind: RemoteDeviceKind,
        active: bool,
        volume: Option<u32>,
    ) -> RemoteDevice {
        RemoteDevice {
            id: id.into(),
            name: name.into(),
            kind,
            active,
            volume_percent: volume,
        }
    }

    fn item(title: &str) -> SourceItem {
        SourceItem {
            source: "org.example.remote".into(),
            kind: ItemKind::Track,
            id: "b".into(),
            title: title.into(),
            subtitle: Some("Sample Artist".into()),
            album: Some("Sample Album".into()),
            duration_ms: Some(90_000),
            artwork: Some(ArtworkRef::Url("https://img.example.com/b.jpg".into())),
            playable: true,
            unplayable_reason: None,
            track: None,
            also_in: Vec::new(),
            link: Some("https://music.example.com/song/b".into()),
            artist_id: None,
            album_id: None,
            downloaded: false,
        }
    }

    /// An invented session: "Sample B" on the desk speaker.
    fn playing() -> RemoteSnapshot {
        let desk = device("desk", "Desk Speaker", RemoteDeviceKind::Speaker, true, Some(40));
        RemoteSnapshot {
            version: 3,
            source: Some("org.example.remote".into()),
            source_name: Some("Example Music".into()),
            status: Status::Playing,
            item: Some(item("Sample B")),
            context: None,
            anchor: PlaybackAnchor {
                track_generation: 1,
                position_us: 12_000_000,
                anchor_mono_ns: 0,
                rate: 1.0,
                duration_us: Some(90_000_000),
            },
            shuffle: true,
            repeat: RepeatMode::One,
            device: Some(desk.clone()),
            devices: vec![
                desk,
                device("phone", "Phone", RemoteDeviceKind::Smartphone, false, Some(100)),
            ],
            error: None,
            quality_note: None,
        }
    }

    fn none() -> RemoteSnapshot {
        RemoteSnapshot {
            version: 4,
            source: None,
            source_name: None,
            status: Status::Stopped,
            item: None,
            context: None,
            anchor: PlaybackAnchor {
                track_generation: 0,
                position_us: 0,
                anchor_mono_ns: 0,
                rate: 0.0,
                duration_us: None,
            },
            shuffle: false,
            repeat: RepeatMode::Off,
            device: None,
            devices: Vec::new(),
            error: None,
            quality_note: None,
        }
    }

    #[test]
    fn no_session_shows_the_engine() {
        assert_eq!(view(&none()), None);
    }

    #[test]
    fn a_session_shows_its_track_and_device() {
        let v = view(&playing()).expect("a session");
        assert_eq!(v.source, "org.example.remote");
        assert_eq!(v.service, "Example Music");
        assert_eq!(v.title.as_deref(), Some("Sample B"));
        assert_eq!(v.artist.as_deref(), Some("Sample Artist"));
        assert_eq!(v.album.as_deref(), Some("Sample Album"));
        assert_eq!(v.duration_ms, Some(90_000));
        assert_eq!(v.link.as_deref(), Some("https://music.example.com/song/b"));
        assert_eq!(v.device.as_deref(), Some("Desk Speaker"));
        assert_eq!(v.device_kind, Some(RemoteDeviceKind::Speaker));
        assert!(v.playing);
        assert!(v.shuffle);
        assert_eq!(v.repeat, RepeatMode::One);
        assert_eq!(v.volume, Some(40));
        assert_eq!(v.error, None);
    }

    #[test]
    fn a_session_without_a_track_or_name_still_shows() {
        let mut slice = playing();
        slice.item = None;
        slice.source_name = Some("  ".into());
        slice.status = Status::Paused;
        slice.error = Some("The service did not answer.".into());
        let v = view(&slice).expect("a session");
        assert_eq!(v.title, None);
        assert_eq!(v.link, None);
        assert_eq!(
            v.service, "org.example.remote",
            "the id stands in for a missing name"
        );
        assert!(!v.playing);
        assert_eq!(v.error.as_deref(), Some("The service did not answer."));
    }

    #[test]
    fn a_device_without_volume_hides_the_control() {
        let mut slice = playing();
        if let Some(d) = slice.device.as_mut() {
            d.volume_percent = None;
        }
        assert_eq!(view(&slice).and_then(|v| v.volume), None);
        slice.device = None;
        let v = view(&slice).expect("a session");
        assert_eq!(v.volume, None);
        assert_eq!(v.device, None);
    }

    #[test]
    fn devices_list_every_device_with_the_playing_one_checked() {
        let rows = device_rows(&playing());
        assert_eq!(
            rows,
            vec![
                DeviceRow {
                    id: "desk".into(),
                    name: "Desk Speaker".into(),
                    glyph: "device-speaker",
                    checked: true,
                },
                DeviceRow {
                    id: "phone".into(),
                    name: "Phone".into(),
                    glyph: "device-phone",
                    checked: false,
                },
            ]
        );
    }

    #[test]
    fn the_playing_device_shows_before_the_list_arrives() {
        let mut slice = playing();
        slice.devices.clear();
        let rows = device_rows(&slice);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].checked);
        // Without a playing device, the one the service marks active.
        let mut slice = playing();
        slice.device = None;
        slice.devices[0].active = false;
        slice.devices[1].active = true;
        let checked: Vec<bool> = device_rows(&slice).iter().map(|r| r.checked).collect();
        assert_eq!(checked, [false, true]);
        // Nameless devices are left out.
        slice
            .devices
            .push(device("ghost", " ", RemoteDeviceKind::Other, false, None));
        assert_eq!(device_rows(&slice).len(), 2);
    }

    #[test]
    fn every_kind_has_a_glyph() {
        for kind in [
            RemoteDeviceKind::Computer,
            RemoteDeviceKind::Smartphone,
            RemoteDeviceKind::Speaker,
            RemoteDeviceKind::Tv,
            RemoteDeviceKind::Car,
            RemoteDeviceKind::Other,
        ] {
            assert!(!device_glyph(kind).is_empty());
        }
    }

    #[test]
    fn an_error_ending_a_session_shows_once() {
        let mut ended = none();
        ended.error = Some("Sign in to Example Music again.".into());
        assert_eq!(
            ended_error(&playing(), &ended).as_deref(),
            Some("Sign in to Example Music again.")
        );
        // The next slice without a session says nothing again.
        assert_eq!(ended_error(&ended, &ended), None);
        // An error while the session runs is not a notice.
        let mut failing = playing();
        failing.error = Some("The service did not answer.".into());
        assert_eq!(ended_error(&playing(), &failing), None);
        // A session ended without an error.
        assert_eq!(ended_error(&playing(), &none()), None);
    }

    #[test]
    fn volumes_stay_within_percent() {
        assert_eq!(volume_percent(0.404), 40);
        assert_eq!(volume_percent(1.7), 100);
        assert_eq!(volume_percent(-1.0), 0);
        assert_eq!(volume_step(40, 5), 45);
        assert_eq!(volume_step(98, 5), 100);
        assert_eq!(volume_step(3, -5), 0);
    }

    #[test]
    fn repeat_cycles() {
        assert_eq!(next_repeat(RepeatMode::Off), RepeatMode::All);
        assert_eq!(next_repeat(RepeatMode::All), RepeatMode::One);
        assert_eq!(next_repeat(RepeatMode::One), RepeatMode::Off);
    }
}
