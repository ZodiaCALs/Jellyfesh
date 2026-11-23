//! The [`Plugin`] trait.

use crate::{
    Capability, CommentPage, DownloadInfo, DownloadQuality, FieldValue, Flow, FlowStep, ImageRef, Item,
    ItemDetails, ItemKind, ItemPage, Look, LyricPayload, Made, MakerPlan, PageRequest, PlaylistPage,
    PluginError, Quality, RemoteDevice, RemoteRepeat, RemoteState, Result, SearchFilters, Shelf, Song,
    StreamInfo,
};

/// A plugin: implement what the manifest declares, leave the rest.
///
/// Every method is an associated function (no `self`): the component has no
/// constructor, and the instance may be replaced between calls. Each default
/// returns [`PluginError::Unsupported`] (`settings_changed` and `logout`
/// succeed by default). Hand the type to [`crate::export_plugin!`].
///
/// Calls run under the host's limits: about 200 ms of CPU (2 s for
/// [`Plugin::auth_begin`]), 10 s of wall-clock time (30 s for login steps),
/// 64 MiB of memory. Waiting for HTTP does not count as CPU.
pub trait Plugin {
    /// What the plugin can do right now (a subset of the manifest's
    /// capabilities, for example after checking the server's features).
    fn capabilities() -> Vec<Capability>;

    /// The settings as a JSON object matching the package's settings schema.
    /// Called after every instantiation, before anything else but
    /// `capabilities`, and whenever the person changes them.
    fn settings_changed(settings: &serde_json::Value) -> Result<()> {
        let _ = settings;
        Ok(())
    }

    /// Starts a login (see the WIT `auth` interface for the flow per kind).
    fn auth_begin(flow: Flow) -> Result<FlowStep> {
        let _ = flow;
        Err(PluginError::Unsupported)
    }

    /// Progress of a polled login step.
    fn auth_poll(flow: Flow) -> Result<FlowStep> {
        let _ = flow;
        Err(PluginError::Unsupported)
    }

    /// Form fields (or, for OAuth, `code`, `code-verifier` and `redirect-uri`).
    fn auth_submit(flow: Flow, fields: Vec<FieldValue>) -> Result<FlowStep> {
        let _ = (flow, fields);
        Err(PluginError::Unsupported)
    }

    /// Revoke the session on the service if it allows; the host deletes the
    /// plugin's secrets and cookies afterwards anyway.
    fn logout() -> Result<()> {
        Ok(())
    }

    /// Full-text search.
    fn search(query: String, filters: SearchFilters, page: PageRequest) -> Result<ItemPage> {
        let _ = (query, filters, page);
        Err(PluginError::Unsupported)
    }

    /// Children of `node`, or the root sections when it is `None`.
    fn browse(node: Option<Item>, page: PageRequest) -> Result<ItemPage> {
        let _ = (node, page);
        Err(PluginError::Unsupported)
    }

    /// Where the engine fetches a track's audio.
    fn resolve_stream(item: Item, quality: Quality) -> Result<StreamInfo> {
        let _ = (item, quality);
        Err(PluginError::Unsupported)
    }

    /// Lyrics for a track (which may come from another source).
    fn lyrics(item: Item) -> Result<Option<LyricPayload>> {
        let _ = item;
        Err(PluginError::Unsupported)
    }

    /// Artwork about `size` pixels on its longer edge.
    fn artwork(item: Item, size: u32) -> Result<Option<ImageRef>> {
        let _ = (item, size);
        Err(PluginError::Unsupported)
    }

    /// Items like `item`.
    fn recommendations_for_item(item: Item, page: PageRequest) -> Result<ItemPage> {
        let _ = (item, page);
        Err(PluginError::Unsupported)
    }

    /// Personal suggestions.
    fn recommendations_for_user(page: PageRequest) -> Result<ItemPage> {
        let _ = page;
        Err(PluginError::Unsupported)
    }

    /// Remote control (API 1.1): the devices the service can play on.
    fn remote_devices() -> Result<Vec<RemoteDevice>> {
        Err(PluginError::Unsupported)
    }

    /// What the service's player is doing; `None` when nothing plays.
    fn remote_state() -> Result<Option<RemoteState>> {
        Err(PluginError::Unsupported)
    }

    /// Plays `tracks` from index `start`, or the album or playlist `context`
    /// from its track `start` (then `tracks` is empty), on `device` (the
    /// active one when `None`).
    fn remote_play(
        device: Option<String>,
        context: Option<Item>,
        tracks: Vec<Item>,
        start: u32,
    ) -> Result<()> {
        let _ = (device, context, tracks, start);
        Err(PluginError::Unsupported)
    }

    /// Resumes playback.
    fn remote_resume() -> Result<()> {
        Err(PluginError::Unsupported)
    }

    /// Pauses playback.
    fn remote_pause() -> Result<()> {
        Err(PluginError::Unsupported)
    }

    /// Skips to the next track.
    fn remote_next() -> Result<()> {
        Err(PluginError::Unsupported)
    }

    /// Goes back to the previous track.
    fn remote_previous() -> Result<()> {
        Err(PluginError::Unsupported)
    }

    /// Seeks within the current track.
    fn remote_seek(position_ms: u32) -> Result<()> {
        let _ = position_ms;
        Err(PluginError::Unsupported)
    }

    /// Sets the active device's volume (0–100).
    fn remote_set_volume(percent: u32) -> Result<()> {
        let _ = percent;
        Err(PluginError::Unsupported)
    }

    /// Turns shuffle on or off.
    fn remote_set_shuffle(on: bool) -> Result<()> {
        let _ = on;
        Err(PluginError::Unsupported)
    }

    /// Sets the repeat mode.
    fn remote_set_repeat(mode: RemoteRepeat) -> Result<()> {
        let _ = mode;
        Err(PluginError::Unsupported)
    }

    /// Moves playback to `device`, playing there if `play`.
    fn remote_transfer(device: String, play: bool) -> Result<()> {
        let _ = (device, play);
        Err(PluginError::Unsupported)
    }

    /// Making lyrics (API 1.2, capability `make_lyrics`): what
    /// [`Plugin::make_lyrics`] would do with the current settings and what
    /// it needs first. Cheap; no network ([`crate::speech::models`] is fine).
    fn maker_plan() -> Result<MakerPlan> {
        Err(PluginError::Unsupported)
    }

    /// Makes synced lyrics for `song`: read its audio with [`crate::audio`]
    /// (online, after consent) or recognise it with [`crate::speech`] (on the
    /// device), then time the words. Runs under its own limits (30 minutes,
    /// 10 s of plugin CPU, 128 MiB) and may be cancelled: return when an
    /// import answers `cancelled`.
    fn make_lyrics(song: Song) -> Result<Made> {
        let _ = song;
        Err(PluginError::Unsupported)
    }

    /// The app's look and layout for the current settings (API 1.2,
    /// capability `appearance`). Leave out what is at its default.
    fn look() -> Result<Look> {
        Err(PluginError::Unsupported)
    }

    /// The source's Discover page (API 1.2, capability `discover`): shelves
    /// of items, at most 12 with 50 items each.
    fn discover() -> Result<Vec<Shelf>> {
        Err(PluginError::Unsupported)
    }

    /// More about an item (API 1.2, capability `details`): notes or a
    /// biography, a large picture, facts and the number of comments.
    fn details(item: Item) -> Result<ItemDetails> {
        let _ = item;
        Err(PluginError::Unsupported)
    }

    /// A page of what people say about an item (API 1.2, capability
    /// `comments`); `hot` only on the first page.
    fn comments(item: Item, page: PageRequest) -> Result<CommentPage> {
        let _ = (item, page);
        Err(PluginError::Unsupported)
    }

    /// The person's liked tracks, albums or artists (`kind`; API 1.3,
    /// capability `library`), newest first where the service keeps that
    /// order. Answer [`PluginError::Unsupported`] for a kind the service
    /// does not keep.
    fn library_likes(kind: ItemKind, page: PageRequest) -> Result<ItemPage> {
        let _ = (kind, page);
        Err(PluginError::Unsupported)
    }

    /// The person's own and followed playlists (API 1.3, capability
    /// `library`); their songs come from [`Plugin::browse`].
    fn library_playlists(page: PageRequest) -> Result<PlaylistPage> {
        let _ = page;
        Err(PluginError::Unsupported)
    }

    /// Where to fetch a track's file at `quality` (API 1.3, capability
    /// `download`). Answer [`PluginError::Unsupported`] when the server does
    /// not let the account download it; never hand out a stream URL that
    /// skips the server's download policy.
    fn download(item: Item, quality: DownloadQuality) -> Result<DownloadInfo> {
        let _ = (item, quality);
        Err(PluginError::Unsupported)
    }
}
