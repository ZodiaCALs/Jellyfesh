//! The [`Plugin`] trait.

use crate::{
    Capability, FieldValue, Flow, FlowStep, ImageRef, Item, ItemPage, LyricPayload, PageRequest, PluginError,
    Quality, RemoteDevice, RemoteRepeat, RemoteState, Result, SearchFilters, StreamInfo,
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
}
