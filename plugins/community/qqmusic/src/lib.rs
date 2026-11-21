//! QQ Music (Unofficial): a community plugin for Jellyfesh.
//!
//! Not made, endorsed or supported by Tencent or by Jellyfesh. It speaks the
//! y.qq.com web player's own interface (u.y.qq.com), with its request
//! signature re-implemented from the protocol, and plays only what the
//! signed-in account may play.
//!
//! * Sign-in: WeChat's QR code, as on the web player ([`login`]).
//! * Search songs, albums, singers and playlists; the library: liked songs
//!   and own playlists; a playlist's and an album's tracks, a singer's
//!   albums ([`catalog`]).
//! * Streams: only files the service grants this account, at the chosen
//!   quality or below; never a preview in place of the song ([`stream`]).
//! * Lyrics: plain synced LRC, with QQ Music's translation merged in as
//!   bilingual lines ([`lyrics`]); never the app-encrypted QRC. Artwork from
//!   the picture service at the size asked for.
//! * Discover (recommended playlists, charts) and details of songs, albums,
//!   playlists and charts ([`discover`]).
//! * Requests: signed ([`sign`]), sent only to u.y.qq.com and, for the QR
//!   sign-in, WeChat's open.weixin.qq.com; streams only from
//!   stream.qqmusic.qq.com subdomains, pictures from y.gtimg.cn and
//!   qpic.y.qq.com.

mod api;
mod catalog;
mod discover;
mod items;
mod login;
mod lyrics;
mod model;
mod settings;
mod sign;
mod stream;

use jf_plugin_sdk::prelude::*;
use serde_json::json;

use crate::items::Id;

struct QqMusic;

/// Text for a plugin error, for people.
pub(crate) fn error_text(error: &PluginError) -> String {
    match error {
        PluginError::Unsupported => "Not supported.".into(),
        PluginError::AuthRequired => "Sign in again.".into(),
        PluginError::NotFound => "Not found.".into(),
        PluginError::InvalidInput(text)
        | PluginError::Network(text)
        | PluginError::Service(text)
        | PluginError::Internal(text) => text.clone(),
        PluginError::RateLimited => "QQ Music asked Jellyfesh to slow down.".into(),
    }
}

/// The album mid of a track: from the item when it has one, else from the
/// song's details.
fn track_album(item: &Item, mid: &str) -> Result<Option<String>> {
    if let Some(Id::Album(album)) = item.album_id.as_deref().and_then(Id::parse) {
        return Ok(Some(album.to_string()));
    }
    let song = catalog::song(mid)?;
    Ok(Some(song.album.mid).filter(|m| !m.is_empty()))
}

impl Plugin for QqMusic {
    fn capabilities() -> Vec<Capability> {
        vec![
            Capability::Auth,
            Capability::Search,
            Capability::Browse,
            Capability::ResolveStream,
            Capability::Lyrics,
            Capability::Artwork,
        ]
    }

    fn settings_changed(settings: &serde_json::Value) -> Result<()> {
        api::set_settings(settings::Settings::parse(settings)?);
        Ok(())
    }

    fn auth_begin(flow: Flow) -> Result<FlowStep> {
        login::begin(flow)
    }

    fn auth_poll(flow: Flow) -> Result<FlowStep> {
        login::poll(flow)
    }

    fn auth_submit(_flow: Flow, _fields: Vec<FieldValue>) -> Result<FlowStep> {
        Ok(FlowStep::failed("Nothing to submit. Start again."))
    }

    fn logout() -> Result<()> {
        // The web player has no sign-out call to make; the host deletes the
        // stored session (and cookies) after this.
        api::forget();
        Ok(())
    }

    fn search(query: String, filters: SearchFilters, page: PageRequest) -> Result<ItemPage> {
        catalog::search(&query, &filters, &page)
    }

    fn browse(node: Option<Item>, page: PageRequest) -> Result<ItemPage> {
        catalog::browse(node.as_ref(), &page)
    }

    fn resolve_stream(item: Item, quality: Quality) -> Result<StreamInfo> {
        stream::resolve(&item, quality)
    }

    fn lyrics(item: Item) -> Result<Option<LyricPayload>> {
        let mid = match Id::parse(&item.id) {
            Some(Id::Track(mid)) => mid.to_string(),
            Some(_) => return Err(PluginError::invalid_input("lyrics are for tracks")),
            None => match catalog::find_song(&item)? {
                Some(song) => song.mid,
                None => return Ok(None),
            },
        };
        let data = api::call(
            "music.musichallSong.PlayLyricInfo",
            "GetPlayLyricInfo",
            json!({ "songMID": mid, "songID": 0, "trans": 1, "roma": 0, "qrc": 0, "crypt": 0 }),
        )?;
        Ok(lyrics::payload(&data, api::settings().translations))
    }

    fn discover() -> Result<Vec<Shelf>> {
        discover::discover()
    }

    fn details(item: Item) -> Result<ItemDetails> {
        discover::details(&item)
    }

    fn artwork(item: Item, size: u32) -> Result<Option<ImageRef>> {
        Ok(match Id::parse(&item.id) {
            Some(Id::Album(mid)) => api::album_picture(mid, size),
            Some(Id::Artist(mid)) => api::singer_picture(mid, size),
            Some(Id::Track(mid)) => {
                track_album(&item, mid)?.and_then(|album| api::album_picture(&album, size))
            }
            Some(Id::Playlist(id)) => {
                let cover = match api::cover_of(&item.id) {
                    Some(cover) => Some(cover),
                    None => catalog::playlist_cover(id)?,
                };
                cover.as_deref().and_then(api::picture)
            }
            Some(Id::Folder(_)) => None,
            None => match item.kind {
                ItemKind::Track => catalog::find_song(&item)?
                    .map(|song| song.album.mid)
                    .filter(|mid| !mid.is_empty())
                    .and_then(|mid| api::album_picture(&mid, size)),
                ItemKind::Album => catalog::find_album(&item)?.and_then(|mid| api::album_picture(&mid, size)),
                _ => None,
            },
        })
    }
}

jf_plugin_sdk::export_plugin!(QqMusic);

#[cfg(test)]
mod tests {
    #[test]
    fn manifest_version_matches_the_crate() {
        let manifest = include_str!("../manifest.toml");
        let line = format!("version = \"{}\"", env!("CARGO_PKG_VERSION"));
        assert!(
            manifest.lines().any(|l| l == line),
            "manifest.toml must say {line}"
        );
    }

    #[test]
    fn manifest_is_not_official() {
        let manifest = include_str!("../manifest.toml");
        assert!(manifest.lines().any(|l| l == "official = false"));
        assert!(!manifest.contains("org.jellyfesh."));
        assert!(manifest.contains("(Unofficial)"));
    }
}
