//! A remote-control test plugin (API 1.1, `playback_mode = "remote"`): two
//! invented devices and a player that only exists in this instance's
//! memory. Some answers are deliberately off, so the tests can see the host
//! correct them: a third device without a name, a volume of 250 % on the
//! phone, and a position past the end of the song after a long seek.

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;

struct Remote;

struct Player {
    device: String,
    tracks: Vec<Item>,
    context: Option<Item>,
    index: usize,
    playing: bool,
    position_ms: u32,
    shuffle: bool,
    repeat: RemoteRepeat,
}

thread_local! {
    static PLAYER: RefCell<Option<Player>> = const { RefCell::new(None) };
    static DESK_VOLUME: RefCell<u32> = const { RefCell::new(40) };
}

fn with_player<R>(f: impl FnOnce(&mut Player) -> R) -> Result<R> {
    PLAYER.with(|p| match p.borrow_mut().as_mut() {
        Some(player) => Ok(f(player)),
        None => Err(PluginError::invalid_input("nothing is playing")),
    })
}

fn device(id: &str, name: &str, kind: RemoteDeviceKind, active: bool, volume: Option<u32>) -> RemoteDevice {
    RemoteDevice {
        id: id.into(),
        name: name.into(),
        kind,
        active,
        supports_volume: volume.is_some(),
        volume_percent: volume,
    }
}

fn devices(active: Option<&str>) -> Vec<RemoteDevice> {
    let desk = DESK_VOLUME.with(|v| *v.borrow());
    vec![
        device(
            "desk",
            "Desk Speaker",
            RemoteDeviceKind::Speaker,
            active == Some("desk"),
            Some(desk),
        ),
        device(
            "phone",
            "Phone",
            RemoteDeviceKind::Smartphone,
            active == Some("phone"),
            Some(250),
        ),
        device("ghost", "", RemoteDeviceKind::Other, false, None),
    ]
}

impl Plugin for Remote {
    fn capabilities() -> Vec<Capability> {
        vec![Capability::Search]
    }

    fn search(query: String, _filters: SearchFilters, _page: PageRequest) -> Result<ItemPage> {
        Ok(ItemPage::new(vec![
            Item::new(ItemKind::Track, "a", format!("{query} A")).duration_ms(60_000),
            Item::new(ItemKind::Track, "b", format!("{query} B")).duration_ms(90_000),
        ]))
    }

    fn remote_devices() -> Result<Vec<RemoteDevice>> {
        let active = PLAYER.with(|p| p.borrow().as_ref().map(|p| p.device.clone()));
        Ok(devices(active.as_deref()))
    }

    fn remote_state() -> Result<Option<RemoteState>> {
        PLAYER.with(|p| {
            Ok(p.borrow().as_ref().map(|player| {
                let all = devices(Some(&player.device));
                RemoteState {
                    device: all.into_iter().find(|d| d.id == player.device),
                    item: player.tracks.get(player.index).cloned(),
                    context: player.context.clone(),
                    playing: player.playing,
                    position_ms: player.position_ms,
                    shuffle: player.shuffle,
                    repeat: player.repeat,
                }
            }))
        })
    }

    fn remote_play(
        device: Option<String>,
        context: Option<Item>,
        tracks: Vec<Item>,
        start: u32,
    ) -> Result<()> {
        let device = device.unwrap_or_else(|| "desk".into());
        if !devices(None).iter().any(|d| d.id == device) {
            return Err(PluginError::NotFound);
        }
        let tracks = match (&context, tracks.is_empty()) {
            (_, false) => tracks,
            (Some(album), true) => vec![
                Item::new(
                    ItemKind::Track,
                    format!("{}-1", album.id),
                    format!("{} 1", album.title),
                ),
                Item::new(
                    ItemKind::Track,
                    format!("{}-2", album.id),
                    format!("{} 2", album.title),
                ),
            ],
            (None, true) => return Err(PluginError::invalid_input("nothing to play")),
        };
        let index = (start as usize).min(tracks.len() - 1);
        PLAYER.with(|p| {
            *p.borrow_mut() = Some(Player {
                device,
                tracks,
                context,
                index,
                playing: true,
                position_ms: 0,
                shuffle: false,
                repeat: RemoteRepeat::Off,
            });
        });
        Ok(())
    }

    fn remote_resume() -> Result<()> {
        with_player(|p| p.playing = true)
    }

    fn remote_pause() -> Result<()> {
        with_player(|p| p.playing = false)
    }

    fn remote_next() -> Result<()> {
        with_player(|p| {
            p.index = (p.index + 1).min(p.tracks.len() - 1);
            p.position_ms = 0;
        })
    }

    fn remote_previous() -> Result<()> {
        with_player(|p| {
            p.index = p.index.saturating_sub(1);
            p.position_ms = 0;
        })
    }

    fn remote_seek(position_ms: u32) -> Result<()> {
        // No check against the song's length: the host must cut it.
        with_player(|p| p.position_ms = position_ms)
    }

    fn remote_set_volume(percent: u32) -> Result<()> {
        if percent > 100 {
            return Err(PluginError::invalid_input("volume above 100"));
        }
        DESK_VOLUME.with(|v| *v.borrow_mut() = percent);
        Ok(())
    }

    fn remote_set_shuffle(on: bool) -> Result<()> {
        with_player(|p| p.shuffle = on)
    }

    fn remote_set_repeat(mode: RemoteRepeat) -> Result<()> {
        with_player(|p| p.repeat = mode)
    }

    fn remote_transfer(device: String, play: bool) -> Result<()> {
        if !devices(None).iter().any(|d| d.id == device) {
            return Err(PluginError::NotFound);
        }
        with_player(|p| {
            p.device = device;
            p.playing = play;
        })
    }
}

jf_plugin_sdk::export_plugin!(Remote);
