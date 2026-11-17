//! Remote control (WIT `remote`) on the Web API's player endpoints. Every
//! command needs Spotify Premium; reading devices and the playback state
//! does not.

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{self, command, get_json};
use crate::items::{Album, Artist, Playlist, Track, uri, valid_id};

/// Most track URIs sent in one play request.
const MAX_URIS: usize = 100;

#[derive(Debug, Clone, Deserialize)]
pub struct Device {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    is_restricted: bool,
    #[serde(default)]
    name: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    volume_percent: Option<u32>,
    #[serde(default)]
    supports_volume: bool,
}

/// Spotify's device types (written "Computer", "Smartphone"… in practice;
/// "computer" in the reference) as Jellyfesh's.
pub fn device_kind(kind: &str) -> RemoteDeviceKind {
    match kind.to_ascii_lowercase().as_str() {
        "computer" => RemoteDeviceKind::Computer,
        "smartphone" | "tablet" => RemoteDeviceKind::Smartphone,
        "speaker" | "avr" | "audiodongle" | "castaudio" => RemoteDeviceKind::Speaker,
        "tv" | "stb" | "castvideo" | "gameconsole" => RemoteDeviceKind::Tv,
        "automobile" => RemoteDeviceKind::Car,
        _ => RemoteDeviceKind::Other,
    }
}

impl Device {
    /// The device, unless Spotify accepts no commands for it (restricted) or
    /// it has no id.
    fn remote(&self) -> Option<RemoteDevice> {
        let id = self.id.clone().filter(|i| !i.is_empty())?;
        if self.is_restricted {
            return None;
        }
        Some(RemoteDevice {
            id,
            name: self.name.trim().to_owned(),
            kind: device_kind(&self.kind),
            active: self.is_active,
            supports_volume: self.supports_volume,
            volume_percent: self.volume_percent,
        })
    }
}

pub fn devices() -> Result<Vec<RemoteDevice>> {
    #[derive(Deserialize)]
    struct Devices {
        #[serde(default)]
        devices: Vec<Device>,
    }
    let answer: Devices = get_json("/me/player/devices")?;
    let mut list: Vec<RemoteDevice> = answer.devices.iter().filter_map(Device::remote).collect();
    list.sort_by_key(|d| !d.active);
    Ok(list)
}

#[derive(Debug, Deserialize)]
struct Context {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    uri: String,
}

#[derive(Debug, Deserialize)]
struct Playback {
    #[serde(default)]
    device: Option<Device>,
    #[serde(default)]
    repeat_state: String,
    #[serde(default)]
    shuffle_state: bool,
    #[serde(default)]
    context: Option<Context>,
    #[serde(default)]
    progress_ms: Option<u64>,
    #[serde(default)]
    is_playing: bool,
    #[serde(default)]
    item: Option<Track>,
}

thread_local! {
    /// Names of contexts already looked up, by URI (a few at most).
    static CONTEXTS: RefCell<Vec<(String, Item)>> = const { RefCell::new(Vec::new()) };
}

/// The album, playlist or artist a context URI names, looked up once (the
/// playback state carries only the URI).
fn context_item(context: &Context) -> Option<Item> {
    if context.uri.ends_with(":collection") || context.kind == "collection" {
        return Some(Item::new(ItemKind::Folder, "tracks", "Liked Songs"));
    }
    let mut parts = context.uri.split(':');
    let (Some("spotify"), Some(kind), Some(id), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    if !valid_id(id) {
        return None;
    }
    if let Some(found) = CONTEXTS.with(|c| {
        c.borrow()
            .iter()
            .find(|(u, _)| *u == context.uri)
            .map(|(_, i)| i.clone())
    }) {
        return Some(found);
    }
    let item = match kind {
        "album" => get_json::<Album>(&format!("/albums/{id}")).ok()?.item(),
        "artist" => get_json::<Artist>(&format!("/artists/{id}")).ok()?.item(),
        "playlist" => get_json::<Playlist>(&format!(
            "/playlists/{id}?fields=id,name,images,owner(display_name)"
        ))
        .ok()?
        .item(),
        _ => None,
    }?;
    CONTEXTS.with(|c| {
        let mut cache = c.borrow_mut();
        if cache.len() >= 8 {
            cache.remove(0);
        }
        cache.push((context.uri.clone(), item.clone()));
    });
    Some(item)
}

pub fn state() -> Result<Option<RemoteState>> {
    let response = api::send(http::Method::Get, "/me/player", None)?;
    if response.status == 204 || (response.is_success() && response.body.is_empty()) {
        return Ok(None);
    }
    if !response.is_success() {
        return Err(api::error(&response, false));
    }
    let playback: Playback = response.json()?;
    let device = playback.device.as_ref().and_then(Device::remote);
    let item = playback.item.as_ref().and_then(|t| t.item(None));
    if device.is_none() && item.is_none() {
        return Ok(None);
    }
    Ok(Some(RemoteState {
        device,
        item,
        context: playback.context.as_ref().and_then(context_item),
        playing: playback.is_playing,
        position_ms: playback.progress_ms.unwrap_or(0).min(u64::from(u32::MAX)) as u32,
        shuffle: playback.shuffle_state,
        repeat: match playback.repeat_state.as_str() {
            "track" => RemoteRepeat::One,
            "context" => RemoteRepeat::All,
            _ => RemoteRepeat::Off,
        },
    }))
}

fn on_device(path: &str, device: Option<&str>) -> Result<String> {
    Ok(match device {
        Some(id) if !id.is_empty() => {
            if id.len() > 128
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            {
                return Err(PluginError::invalid_input("not a Spotify device id"));
            }
            format!("{path}?device_id={id}")
        }
        _ => path.to_owned(),
    })
}

/// The body of a play request (`context` from track `start`, or `tracks`).
pub fn play_body(context: Option<&Item>, tracks: &[Item], start: u32) -> Result<Value> {
    if let Some(context) = context {
        return match context.kind {
            ItemKind::Album | ItemKind::Playlist => {
                let uri = uri(context).ok_or(PluginError::NotFound)?;
                Ok(json!({ "context_uri": uri, "offset": { "position": start } }))
            }
            // Artists play from their top; Spotify takes no offset there.
            ItemKind::Artist => Ok(json!({ "context_uri": uri(context).ok_or(PluginError::NotFound)? })),
            _ => Err(PluginError::invalid_input(
                "Spotify plays albums, playlists and artists as a whole",
            )),
        };
    }
    let uris: Vec<String> = tracks
        .iter()
        .filter_map(|t| (t.kind == ItemKind::Track).then(|| uri(t)).flatten())
        .collect();
    if uris.is_empty() {
        return Err(PluginError::invalid_input("nothing to play"));
    }
    // A window of at most MAX_URIS that holds the start track.
    let start = (start as usize).min(uris.len() - 1);
    let first = start
        .saturating_sub(MAX_URIS / 4)
        .min(uris.len().saturating_sub(MAX_URIS));
    let window: Vec<&String> = uris.iter().skip(first).take(MAX_URIS).collect();
    Ok(json!({ "uris": window, "offset": { "position": start - first } }))
}

pub fn play(device: Option<String>, context: Option<Item>, tracks: Vec<Item>, start: u32) -> Result<()> {
    let body = match &context {
        // Liked Songs has no context URI: play its tracks from `start`.
        Some(folder) if folder.kind == ItemKind::Folder && folder.id == "tracks" => {
            #[derive(Deserialize)]
            struct Saved {
                #[serde(default)]
                items: Vec<SavedTrack>,
            }
            #[derive(Deserialize)]
            struct SavedTrack {
                #[serde(default)]
                track: Option<Track>,
            }
            let saved: Saved = get_json(&format!("/me/tracks?limit=50&offset={start}"))?;
            let tracks: Vec<Item> = saved
                .items
                .iter()
                .filter_map(|s| s.track.as_ref().and_then(|t| t.item(None)))
                .collect();
            play_body(None, &tracks, 0)?
        }
        _ => play_body(context.as_ref(), &tracks, start)?,
    };
    command(
        http::Method::Put,
        &on_device("/me/player/play", device.as_deref())?,
        Some(&body),
    )
}

pub fn transfer(device: String, play: bool) -> Result<()> {
    if device.is_empty() {
        return Err(PluginError::invalid_input("no device"));
    }
    command(
        http::Method::Put,
        "/me/player",
        Some(&json!({ "device_ids": [device], "play": play })),
    )
}

pub fn simple(method: http::Method, path: &str) -> Result<()> {
    command(method, path, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Item {
        Item::new(ItemKind::Track, id, id)
    }

    #[test]
    fn devices_map_and_restricted_ones_are_hidden() {
        let d: Device = serde_json::from_value(json!({
            "id": "dev1", "is_active": true, "is_restricted": false, "name": " Kitchen ", "type": "Speaker",
            "volume_percent": 40, "supports_volume": true
        }))
        .unwrap();
        let remote = d.remote().unwrap();
        assert_eq!(
            (remote.name.as_str(), remote.kind, remote.active),
            ("Kitchen", RemoteDeviceKind::Speaker, true)
        );
        let restricted: Device =
            serde_json::from_value(json!({ "id": "dev2", "is_restricted": true, "name": "TV" })).unwrap();
        assert!(restricted.remote().is_none());
        let nameless: Device = serde_json::from_value(json!({ "id": null, "name": "Ghost" })).unwrap();
        assert!(nameless.remote().is_none());
        assert_eq!(device_kind("computer"), RemoteDeviceKind::Computer);
        assert_eq!(device_kind("Automobile"), RemoteDeviceKind::Car);
        assert_eq!(device_kind("GameConsole"), RemoteDeviceKind::Tv);
        assert_eq!(device_kind("Watch"), RemoteDeviceKind::Other);
    }

    #[test]
    fn play_bodies_follow_the_reference() {
        let mut album = Item::new(ItemKind::Album, "6akEvsycLGftJxYudPjmqK", "Night Ferry");
        assert_eq!(
            play_body(Some(&album), &[], 4).unwrap(),
            json!({ "context_uri": "spotify:album:6akEvsycLGftJxYudPjmqK", "offset": { "position": 4 } })
        );
        album.kind = ItemKind::Artist;
        assert_eq!(
            play_body(Some(&album), &[], 4).unwrap(),
            json!({ "context_uri": "spotify:artist:6akEvsycLGftJxYudPjmqK" })
        );
        let tracks = vec![
            track("a1"),
            track("b2"),
            Item::new(ItemKind::Folder, "tracks", "x"),
        ];
        assert_eq!(
            play_body(None, &tracks, 1).unwrap(),
            json!({ "uris": ["spotify:track:a1", "spotify:track:b2"], "offset": { "position": 1 } })
        );
        assert!(play_body(None, &[], 0).is_err());
        assert!(play_body(Some(&Item::new(ItemKind::Folder, "albums", "x")), &[], 0).is_err());
        // A long list sends a window that holds the start.
        let many: Vec<Item> = (0..300).map(|i| track(&format!("t{i}"))).collect();
        let body = play_body(None, &many, 250).unwrap();
        let uris = body["uris"].as_array().unwrap();
        assert_eq!(uris.len(), MAX_URIS);
        let at = body["offset"]["position"].as_u64().unwrap() as usize;
        assert_eq!(uris[at], "spotify:track:t250");
    }
}
