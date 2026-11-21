//! NetEase Cloud Music (Unofficial): a community plugin for Jellyfesh.
//!
//! Not made, endorsed or supported by NetEase or by Jellyfesh. It speaks the
//! web player's own interface (the "weapi" endpoints of music.163.com),
//! re-implemented from the protocol, and plays only what the signed-in
//! account may play.
//!
//! * Sign-in: the web player's QR code, scanned with the NetEase Cloud Music
//!   app ([`login`]).
//! * Search songs, albums, artists and playlists; the library: liked songs,
//!   own and saved playlists; a playlist's, an album's tracks, an artist's
//!   albums ([`catalog`]).
//! * Streams: only full tracks the account may play, at the chosen quality;
//!   never a preview in place of the song ([`stream`]).
//! * Lyrics: word by word when NetEase has word timing (its YRC, converted
//!   to enhanced LRC in [`yrc`]), else synced LRC, with NetEase's
//!   translation merged in as bilingual lines ([`lyrics`]). Artwork at the
//!   size asked for.
//! * Discover (daily picks, recommended playlists, charts), details of
//!   songs, albums, artists and playlists, and their comments, read-only
//!   ([`discover`]).
//! * Requests: weapi encoding ([`weapi`], with the raw RSA step in
//!   [`rsa_raw`]), sent only to music.163.com; streams and pictures only from
//!   music.126.net subdomains.

mod api;
mod catalog;
mod discover;
mod items;
mod login;
mod lyrics;
mod model;
mod rsa_raw;
mod settings;
mod stream;
mod weapi;
mod yrc;
#[cfg(test)]
mod yrc_golden;

use jf_plugin_sdk::prelude::*;
use serde_json::json;

use crate::items::Id;

struct NetEase;

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
        PluginError::RateLimited => "NetEase Cloud Music asked Jellyfesh to slow down.".into(),
    }
}

/// The picture URL behind one of the plugin's items.
fn own_cover(id: Id<'_>, item_id: &str) -> Result<Option<String>> {
    if let Some(cover) = api::cover_of(item_id) {
        return Ok(Some(cover));
    }
    let cover = match id {
        Id::Track(id) => catalog::song(id)?.al.and_then(|a| a.pic_url),
        Id::Album(id) => catalog::album(id)?.0.pic_url,
        Id::Artist(id) => {
            let artist = catalog::artist(id)?;
            artist.pic_url.or(artist.img1v1_url)
        }
        Id::Playlist(id) => catalog::playlist(id)?.cover().map(str::to_owned),
        Id::Folder(_) => None,
    };
    api::remember_cover(item_id, cover.as_deref());
    Ok(cover)
}

/// The picture URL for an item from another source, found by search.
fn foreign_cover(item: &Item) -> Result<Option<String>> {
    Ok(match item.kind {
        ItemKind::Track => catalog::find_song(item)?
            .and_then(|s| s.al)
            .and_then(|a| a.pic_url),
        ItemKind::Album => catalog::find_album(item)?.and_then(|a| a.pic_url),
        _ => None,
    })
}

impl Plugin for NetEase {
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
        // End the session on NetEase too; the host deletes the stored
        // session and the cookies whatever this answers.
        if api::session().ok().flatten().is_some()
            && let Err(error) = api::call("/logout", json!({}))
        {
            jf_plugin_sdk::info!("signing out on NetEase did not work: {}", error_text(&error));
        }
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
        let id = match Id::parse(&item.id) {
            Some(Id::Track(id)) => id,
            Some(_) => return Err(PluginError::invalid_input("lyrics are for tracks")),
            None => match catalog::find_song(&item)? {
                Some(song) => song.id,
                None => return Ok(None),
            },
        };
        Ok(lyrics::payload(&lyrics::fetch(id)?, api::settings().translations))
    }

    fn discover() -> Result<Vec<Shelf>> {
        discover::discover()
    }

    fn details(item: Item) -> Result<ItemDetails> {
        discover::details(&item)
    }

    fn comments(item: Item, page: PageRequest) -> Result<CommentPage> {
        discover::comments(&item, &page)
    }

    fn artwork(item: Item, size: u32) -> Result<Option<ImageRef>> {
        let cover = match Id::parse(&item.id) {
            Some(id) => own_cover(id, &item.id)?,
            None => foreign_cover(&item)?,
        };
        Ok(cover.and_then(|url| api::image(&url, size)))
    }
}

jf_plugin_sdk::export_plugin!(NetEase);

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
