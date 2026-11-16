//! `From` conversions between the `jf-core` types and their mirrors, in both directions.
//!
//! Structs convert field by field through [`Conv`], so both directions name every field: a field
//! added to either side without the other fails to compile. Enums convert with exhaustive
//! matches. Paths that are not UTF-8 are converted lossily (the library skips such files anyway).

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::path::PathBuf;
use std::sync::Arc;

use jf_core as jf;

use crate::types::*;

/// A value conversion used field by field; [`From`] for the mirror types is built on it.
pub(crate) trait Conv<T> {
    fn conv(self) -> T;
}

macro_rules! identity {
    ($($t:ty),*) => {
        $(impl Conv<$t> for $t {
            fn conv(self) -> $t {
                self
            }
        })*
    };
}
identity!((), bool, u8, u16, u32, u64, i32, i64, f32, String);

impl Conv<String> for PathBuf {
    fn conv(self) -> String {
        self.to_string_lossy().into_owned()
    }
}

impl Conv<PathBuf> for String {
    fn conv(self) -> PathBuf {
        PathBuf::from(self)
    }
}

/// Counts and indices cross as `u32`; larger values saturate.
impl Conv<u32> for usize {
    fn conv(self) -> u32 {
        u32::try_from(self).unwrap_or(u32::MAX)
    }
}

impl Conv<usize> for u32 {
    fn conv(self) -> usize {
        self as usize
    }
}

impl<A: Conv<B>, B> Conv<Option<B>> for Option<A> {
    fn conv(self) -> Option<B> {
        self.map(Conv::conv)
    }
}

impl<A: Conv<B>, B> Conv<Vec<B>> for Vec<A> {
    fn conv(self) -> Vec<B> {
        self.into_iter().map(Conv::conv).collect()
    }
}

/// Shared slices are flattened: the value is cloned out of the `Arc` unless this was the last
/// reference.
impl<A: Conv<B> + Clone, B> Conv<B> for Arc<A> {
    fn conv(self) -> B {
        Arc::unwrap_or_clone(self).conv()
    }
}

impl<K: Ord + Hash + Eq, A: Conv<B>, B> Conv<HashMap<K, B>> for BTreeMap<K, A> {
    fn conv(self) -> HashMap<K, B> {
        self.into_iter().map(|(k, v)| (k, v.conv())).collect()
    }
}

impl<K: Ord + Hash + Eq, A: Conv<B>, B> Conv<BTreeMap<K, B>> for HashMap<K, A> {
    fn conv(self) -> BTreeMap<K, B> {
        self.into_iter().map(|(k, v)| (k, v.conv())).collect()
    }
}

/// `SubmitLogin` fields: pairs in the core, a map across the FFI.
impl Conv<HashMap<String, String>> for Vec<(String, String)> {
    fn conv(self) -> HashMap<String, String> {
        self.into_iter().collect()
    }
}

/// Map to pairs, sorted by field name so the order is stable.
impl Conv<Vec<(String, String)>> for HashMap<String, String> {
    fn conv(self) -> Vec<(String, String)> {
        let mut pairs: Vec<(String, String)> = self.into_iter().collect();
        pairs.sort();
        pairs
    }
}

impl Conv<ScanError> for (PathBuf, String) {
    fn conv(self) -> ScanError {
        ScanError {
            path: self.0.conv(),
            message: self.1,
        }
    }
}

impl Conv<(PathBuf, String)> for ScanError {
    fn conv(self) -> (PathBuf, String) {
        (self.path.conv(), self.message)
    }
}

/// Theme colours: a name-ordered map of `[r, g, b, a]` in the core, `0xRRGGBBAA` records here.
impl Conv<Vec<ThemeColor>> for BTreeMap<String, [u8; 4]> {
    fn conv(self) -> Vec<ThemeColor> {
        self.into_iter()
            .map(|(name, rgba)| ThemeColor {
                name,
                rgba: u32::from_be_bytes(rgba),
            })
            .collect()
    }
}

impl Conv<BTreeMap<String, [u8; 4]>> for Vec<ThemeColor> {
    fn conv(self) -> BTreeMap<String, [u8; 4]> {
        self.into_iter().map(|c| (c.name, c.rgba.to_be_bytes())).collect()
    }
}

/// `Conv` in both directions for a pair of types that already convert with `From`.
macro_rules! conv_via_from {
    ($($core:ty => $ffi:ty),* $(,)?) => {
        $(
            impl Conv<$ffi> for $core {
                fn conv(self) -> $ffi {
                    self.into()
                }
            }
            impl Conv<$core> for $ffi {
                fn conv(self) -> $core {
                    self.into()
                }
            }
        )*
    };
}

/// `From` in both directions for a struct and its mirror, field by field.
macro_rules! mirror_struct {
    ($core:path => $ffi:ident { $($field:ident),* $(,)? }) => {
        impl From<$core> for $ffi {
            fn from(value: $core) -> Self {
                $ffi { $($field: value.$field.conv()),* }
            }
        }
        impl From<$ffi> for $core {
            fn from(value: $ffi) -> Self {
                $core { $($field: value.$field.conv()),* }
            }
        }
        conv_via_from!($core => $ffi);
    };
}

/// `From` in both directions for an enum and its mirror whose variants have the same names and
/// fields. `$core` must be a plain identifier (import the core enum under an alias).
macro_rules! mirror_enum {
    ($core:ident => $ffi:ident { $($variant:ident $({ $($field:ident),* })?),* $(,)? }) => {
        impl From<$core> for $ffi {
            fn from(value: $core) -> Self {
                match value {
                    $($core::$variant $({ $($field),* })? => $ffi::$variant $({ $($field: $field.conv()),* })?,)*
                }
            }
        }
        impl From<$ffi> for $core {
            fn from(value: $ffi) -> Self {
                match value {
                    $($ffi::$variant $({ $($field),* })? => $core::$variant $({ $($field: $field.conv()),* })?,)*
                }
            }
        }
        conv_via_from!($core => $ffi);
    };
}

/// Mirrors that the core shares behind an `Arc`.
macro_rules! into_arc {
    ($($ffi:ty => $core:ty),* $(,)?) => {
        $(impl Conv<Arc<$core>> for $ffi {
            fn conv(self) -> Arc<$core> {
                Arc::new(self.into())
            }
        })*
    };
}

// ---- setup --------------------------------------------------------------------------------------

use jf::OutputKind as CoreOutputKind;
mirror_enum!(CoreOutputKind => OutputKind {
    Device { name },
    WavFile { path, realtime },
    Null { realtime },
});

/// The core's `Config` beyond these fields (the plugin host's options, the secret store) keeps
/// its defaults; `JellyfeshCore::with_secret_store` sets the store.
impl From<CoreConfig> for jf::Config {
    fn from(value: CoreConfig) -> Self {
        jf::Config {
            output: value.output.conv(),
            buffer_ms: value.buffer_ms,
            restore_session: value.restore_session,
            persist_session: value.persist_session,
            watch_folders: value.watch_folders,
            bundled_plugins: value.bundled_plugins.conv(),
            ..jf::Config::new(value.data_dir, value.cache_dir)
        }
    }
}

impl From<jf::Config> for CoreConfig {
    fn from(value: jf::Config) -> Self {
        CoreConfig {
            data_dir: value.data_dir.conv(),
            cache_dir: value.cache_dir.conv(),
            output: value.output.conv(),
            buffer_ms: value.buffer_ms,
            restore_session: value.restore_session,
            persist_session: value.persist_session,
            watch_folders: value.watch_folders,
            bundled_plugins: value.bundled_plugins.conv(),
        }
    }
}
conv_via_from!(jf::Config => CoreConfig);

// ---- commands -----------------------------------------------------------------------------------

use jf::Command as CoreCommand;
mirror_enum!(CoreCommand => Command {
    PlayTracks { tracks, start },
    Enqueue { tracks, play_next },
    RemoveFromQueue { uid },
    MoveInQueue { uid, to },
    JumpTo { uid },
    ClearQueue,
    SetShuffle { on },
    SetRepeat { mode },
    Play,
    Pause,
    TogglePlayPause,
    Stop,
    Next,
    Previous,
    Seek { position_ms },
    SetVolume { volume },
    SetEq { eq },
    SetReplayGain { mode, preamp_db },
    AddRoot { path },
    RemoveRoot { path },
    Rescan,
    PlayCollection { collection, start, shuffle },
    EnqueueCollection { collection, play_next },
    SetSettings { settings },
    SetMuted { muted },
    SetCrossfade { crossfade },
    SetOutputDevice { name },
    RefreshDevices,
    CreatePlaylist { name, tracks },
    RenamePlaylist { id, name },
    DeletePlaylist { id },
    AddToPlaylist { id, tracks },
    RemoveFromPlaylist { id, entries },
    MovePlaylistEntry { id, entry, to },
    ImportPlaylist { path },
    SetFavorite { track, favorite },
    InstallPlugin { path },
    UninstallPlugin { id },
    SetPluginEnabled { id, enabled },
    SetPluginSettings { id, settings_json },
    AcknowledgeUnofficial { id },
    BeginLogin { plugin, method },
    ConnectSource { preset, address, method },
    SubmitLogin { plugin, flow, fields },
    CancelLogin { plugin, flow },
    Logout { plugin },
    Search { query, scope },
    Browse { plugin, node, page },
    Remote { action },
});

use jf::RemoteAction as CoreRemoteAction;
mirror_enum!(CoreRemoteAction => RemoteAction {
    Refresh,
    Transfer { device },
    SetVolume { percent },
    End,
});

impl From<jf::TrackRef> for TrackRef {
    fn from(value: jf::TrackRef) -> Self {
        match value {
            jf::TrackRef::Library(id) => TrackRef::Library { id },
            jf::TrackRef::File(path) => TrackRef::File { path: path.conv() },
            jf::TrackRef::Source(track) => TrackRef::Source { track: track.into() },
        }
    }
}

impl From<TrackRef> for jf::TrackRef {
    fn from(value: TrackRef) -> Self {
        match value {
            TrackRef::Library { id } => jf::TrackRef::Library(id),
            TrackRef::File { path } => jf::TrackRef::File(path.conv()),
            TrackRef::Source { track } => jf::TrackRef::Source(track.into()),
        }
    }
}
conv_via_from!(jf::TrackRef => TrackRef);

mirror_struct!(jf_session::SourceTrack => SourceTrack {
    plugin,
    id,
    title,
    artist,
    album,
    duration_ms,
    artwork_url,
    isrc,
});

use jf::Collection as CoreCollection;
mirror_enum!(CoreCollection => Collection {
    Album { id },
    Artist { id },
    Playlist { id },
    Favorites,
    Tracks { tracks },
    Source { plugin, id },
});

use jf::SearchScope as CoreSearchScope;
mirror_enum!(CoreSearchScope => SearchScope { All, Library, Plugin { id } });

use jf::AuthMethod as CoreAuthMethod;
mirror_enum!(CoreAuthMethod => AuthMethod { Qr, Oauth, CookieImport, PasswordExchange, ApiKey });

use jf::RepeatMode as CoreRepeatMode;
mirror_enum!(CoreRepeatMode => RepeatMode { Off, All, One });

// ---- settings -----------------------------------------------------------------------------------

mirror_struct!(jf::CrossfadeSettings => CrossfadeSettings { enabled, duration_ms, album_continuity });

use jf::AppearancePref as CoreAppearancePref;
mirror_enum!(CoreAppearancePref => AppearancePref { System, Light, Dark });

use jf::BackdropStyle as CoreBackdropStyle;
mirror_enum!(CoreBackdropStyle => BackdropStyle { Artwork, Misty });

use jf::HueSource as CoreHueSource;
mirror_enum!(CoreHueSource => HueSource { Artwork, Fixed { hue } });

use jf::LyricsSize as CoreLyricsSize;
mirror_enum!(CoreLyricsSize => LyricsSize { Small, Medium, Large });

use jf::LyricsAlign as CoreLyricsAlign;
mirror_enum!(CoreLyricsAlign => LyricsAlign { Start, Center });

mirror_struct!(jf::Settings => Settings {
    appearance,
    style,
    app_icon,
    hue_source,
    reduce_effects,
    transparency,
    lyrics_size,
    lyrics_align,
    show_translation,
    show_romanization,
    crossfade,
    replaygain_mode,
    preamp_db,
    eq,
    output_device,
    resume_on_launch,
    watch_folders,
    allow_community_plugins,
});

use jf::ReplayGainMode as CoreReplayGainMode;
mirror_enum!(CoreReplayGainMode => ReplayGainMode { Off, Track, Album });

mirror_struct!(jf::EqSettings => EqSettings { enabled, preamp_db, bands });
mirror_struct!(jf::EqBand => EqBand { kind, freq_hz, gain_db, q, enabled });
mirror_struct!(jf::EqPreset => EqPreset { id, settings });

use jf::BandKind as CoreBandKind;
mirror_enum!(CoreBandKind => BandKind { Peaking, LowShelf, HighShelf });

// ---- devices and sources ------------------------------------------------------------------------

mirror_struct!(jf::DeviceInfo => DeviceInfo { id, name, is_default, sample_rates });
mirror_struct!(jf::DevicesSnapshot => DevicesSnapshot { version, devices, selected });

use jf::PluginBadge as CorePluginBadge;
mirror_enum!(CorePluginBadge => PluginBadge { Official, Community });

mirror_struct!(jf::PluginSummary => PluginSummary {
    id,
    name,
    version,
    author,
    description,
    badge,
    signing_key,
    vendor_playback,
    remote_playback,
    capabilities,
    auth_methods,
    enabled,
    disabled_reason,
    account,
    needs_disclosure,
    attribution,
    settings_schema_json,
    settings_json,
    icon,
});

use jf::SecretStorage as CoreSecretStorage;
mirror_enum!(CoreSecretStorage => SecretStorage { Keystore, SessionOnly });

mirror_struct!(jf::SourcePreset => SourcePreset {
    id,
    name,
    plugin,
    auth_methods,
    needs_address,
    address,
    address_example,
    account,
});

mirror_struct!(jf::SourcesSnapshot => SourcesSnapshot {
    version,
    plugins,
    presets,
    allow_community,
    secret_storage,
    unavailable,
});

use jf::AuthFieldKind as CoreAuthFieldKind;
mirror_enum!(CoreAuthFieldKind => AuthFieldKind { Text, Email, Url, Number, CookieHeader });

mirror_struct!(jf::AuthField => AuthField { name, label, kind, secret, required, placeholder, help });

use jf::AuthStep as CoreAuthStep;
mirror_enum!(CoreAuthStep => AuthStep {
    ShowQr { data, hint, expires_at_ms },
    ShowCode { code, hint, expires_at_ms },
    OpenUrl { url },
    AskFields { fields },
    Waiting { message },
    Done { account },
    Failed { message },
});

mirror_struct!(jf::AuthUpdate => AuthUpdate { plugin, flow, step });

// ---- search, browse and lists -------------------------------------------------------------------

use jf::ItemKind as CoreItemKind;
mirror_enum!(CoreItemKind => ItemKind { Track, Album, Artist, Playlist, Folder });

impl From<jf::ArtworkRef> for ArtworkRef {
    fn from(value: jf::ArtworkRef) -> Self {
        match value {
            jf::ArtworkRef::File(path) => ArtworkRef::File { path: path.conv() },
            jf::ArtworkRef::Url(url) => ArtworkRef::Url { url },
        }
    }
}

impl From<ArtworkRef> for jf::ArtworkRef {
    fn from(value: ArtworkRef) -> Self {
        match value {
            ArtworkRef::File { path } => jf::ArtworkRef::File(path.conv()),
            ArtworkRef::Url { url } => jf::ArtworkRef::Url(url),
        }
    }
}
conv_via_from!(jf::ArtworkRef => ArtworkRef);

mirror_struct!(jf::SourceItem => SourceItem {
    source,
    kind,
    id,
    title,
    subtitle,
    album,
    duration_ms,
    artwork,
    playable,
    unplayable_reason,
    track,
    also_in,
    link,
});

impl Conv<u64> for jf::CommandId {
    fn conv(self) -> u64 {
        self.0
    }
}

impl Conv<jf::CommandId> for u64 {
    fn conv(self) -> jf::CommandId {
        jf::CommandId(self)
    }
}

mirror_struct!(jf::SearchUpdate => SearchUpdate { request, query, source, items, error, done });
mirror_struct!(jf::BrowseUpdate => BrowseUpdate { request, plugin, node, items, next_page, error });
mirror_struct!(jf::TrackRow => TrackRow {
    track,
    title,
    artist,
    album,
    duration_ms,
    artwork,
    artwork_hash,
    favorite,
    missing,
});
mirror_struct!(jf::PlaylistSummary => PlaylistSummary {
    id,
    name,
    entry_count,
    duration_ms,
    updated_at_ms,
    artwork_hash,
});
mirror_struct!(jf::PlaylistEntry => PlaylistEntry { entry_id, position, row });

// ---- library ------------------------------------------------------------------------------------

mirror_struct!(jf::Track => Track {
    id,
    path,
    title,
    artist,
    album,
    album_artist,
    album_id,
    track_no,
    disc_no,
    year,
    genre,
    duration_ms,
    sample_rate,
    channels,
    bit_depth,
    codec,
    replaygain,
    isrc,
    has_embedded_lyrics,
    artwork_hash,
    file_size,
    mtime_ns,
});
mirror_struct!(jf_library::ReplayGain => ReplayGain {
    track_gain_db,
    track_peak,
    album_gain_db,
    album_peak,
});
mirror_struct!(jf::Album => Album { id, title, album_artist, year, track_count, artwork_hash });
mirror_struct!(jf::Artist => Artist { id, name, track_count });

use jf::TrackSort as CoreTrackSort;
mirror_enum!(CoreTrackSort => TrackSort { Artist, Title, Album, Year, Duration, Path, Added });

use jf::AlbumSort as CoreAlbumSort;
mirror_enum!(CoreAlbumSort => AlbumSort { Title, Artist, Year, Added });

mirror_struct!(jf::TrackQuery => TrackQuery { offset, limit, sort, descending, album_id, artist_id });
mirror_struct!(jf::SearchResults => SearchResults { tracks, albums, artists });
mirror_struct!(jf::LibraryStats => LibraryStats {
    roots,
    tracks,
    albums,
    artists,
    artwork,
    total_duration_ms,
    total_bytes,
});
mirror_struct!(jf::ScanProgress => ScanProgress { files_seen, files_changed, current });
mirror_struct!(jf::ScanReport => ScanReport { added, updated, removed, unchanged, errors });

impl From<jf::ScanStatus> for ScanStatus {
    fn from(value: jf::ScanStatus) -> Self {
        match value {
            jf::ScanStatus::Progress(progress) => ScanStatus::Progress {
                progress: progress.into(),
            },
            jf::ScanStatus::Finished(report) => ScanStatus::Finished {
                report: report.into(),
            },
            jf::ScanStatus::Failed(message) => ScanStatus::Failed { message },
        }
    }
}

impl From<ScanStatus> for jf::ScanStatus {
    fn from(value: ScanStatus) -> Self {
        match value {
            ScanStatus::Progress { progress } => jf::ScanStatus::Progress(progress.into()),
            ScanStatus::Finished { report } => jf::ScanStatus::Finished(report.into()),
            ScanStatus::Failed { message } => jf::ScanStatus::Failed(message),
        }
    }
}

// ---- playback and queue -------------------------------------------------------------------------

use jf::Status as CoreStatus;
mirror_enum!(CoreStatus => Status { Stopped, Playing, Paused, Ended });

mirror_struct!(jf::PlaybackAnchor => PlaybackAnchor {
    track_generation,
    position_us,
    anchor_mono_ns,
    rate,
    duration_us,
});
mirror_struct!(jf::PlaybackSnapshot => PlaybackSnapshot {
    version,
    status,
    anchor,
    volume,
    eq,
    replaygain_mode,
    preamp_db,
});
mirror_struct!(jf::QueueItem => QueueItem { uid, track });
mirror_struct!(jf::QueueSnapshot => QueueSnapshot {
    version,
    len,
    position,
    current,
    shuffle,
    repeat,
    up_next,
});
mirror_struct!(jf::HistoryEntry => HistoryEntry { track, played_at_ms });
mirror_struct!(jf::TrackInfo => TrackInfo {
    codec,
    sample_rate,
    channels,
    duration_us,
    gapless,
    bitrate,
});
mirror_struct!(jf::NowPlaying => NowPlaying {
    uid,
    track,
    path,
    title,
    artist,
    album,
    duration_ms,
    artwork,
    format,
    link,
});

// ---- remote sessions ----------------------------------------------------------------------------

use jf::RemoteDeviceKind as CoreRemoteDeviceKind;
mirror_enum!(CoreRemoteDeviceKind => RemoteDeviceKind { Computer, Smartphone, Speaker, Tv, Car, Other });

mirror_struct!(jf::RemoteDevice => RemoteDevice { id, name, kind, active, volume_percent });

mirror_struct!(jf::RemoteSnapshot => RemoteSnapshot {
    version,
    source,
    source_name,
    status,
    item,
    context,
    anchor,
    shuffle,
    repeat,
    device,
    devices,
    error,
});

// ---- lyrics -------------------------------------------------------------------------------------

mirror_struct!(jf::LyricDoc => LyricDoc {
    language,
    timing,
    offset_ms,
    agents,
    lines,
    tracks,
    source,
    metadata,
});

use jf_lyrics::Timing as CoreTiming;
mirror_enum!(CoreTiming => Timing { Unsynced, Line, Word });

mirror_struct!(jf_lyrics::Agent => Agent { id, name, kind });

use jf_lyrics::AgentKind as CoreAgentKind;
mirror_enum!(CoreAgentKind => AgentKind { Person, Group, Other });

mirror_struct!(jf::Line => Line { id, start_ms, end_ms, agent, text, words, background, direction });
mirror_struct!(jf::Word => Word { start_ms, end_ms, text, joins_next });

use jf_lyrics::Direction as CoreDirection;
mirror_enum!(CoreDirection => Direction { Ltr, Rtl, Auto });

mirror_struct!(jf::AuxTrack => AuxTrack { kind, language, lines });

use jf::AuxKind as CoreAuxKind;
mirror_enum!(CoreAuxKind => AuxKind { Translation, Romanization });

mirror_struct!(jf_lyrics::AuxLine => AuxLine { text, words });
mirror_struct!(jf_lyrics::SourceInfo => SourceInfo { format, origin });

use jf_lyrics::Format as CoreFormat;
mirror_enum!(CoreFormat => Format { Lrc, EnhancedLrc, Ttml, WebVtt, Plain });

mirror_struct!(jf::LyricTimeline => LyricTimeline {
    line_starts,
    line_ends,
    segment_starts,
    segment_lines,
    word_offsets,
    word_starts,
    word_ends,
});

impl From<jf::LyricsSource> for LyricsSource {
    fn from(value: jf::LyricsSource) -> Self {
        match value {
            jf::LyricsSource::Sidecar(path) => LyricsSource::Sidecar { path: path.conv() },
            jf::LyricsSource::Embedded => LyricsSource::Embedded,
            jf::LyricsSource::Plugin(id) => LyricsSource::Plugin { id },
        }
    }
}

impl From<LyricsSource> for jf::LyricsSource {
    fn from(value: LyricsSource) -> Self {
        match value {
            LyricsSource::Sidecar { path } => jf::LyricsSource::Sidecar(path.conv()),
            LyricsSource::Embedded => jf::LyricsSource::Embedded,
            LyricsSource::Plugin { id } => jf::LyricsSource::Plugin(id),
        }
    }
}
conv_via_from!(jf::LyricsSource => LyricsSource);

mirror_struct!(jf::LyricsSnapshot => LyricsSnapshot { uid, doc, timeline, source });

// ---- theme --------------------------------------------------------------------------------------

use jf::Appearance as CoreAppearance;
mirror_enum!(CoreAppearance => Appearance { Light, Dark });

mirror_struct!(jf::Theme => Theme { hue, appearance, colors });
mirror_struct!(jf::ThemeSnapshot => ThemeSnapshot {
    uid,
    hue,
    from_artwork,
    light,
    dark,
    backdrop_light,
    backdrop_dark,
});

// ---- snapshot and events ------------------------------------------------------------------------

into_arc!(
    NowPlaying => jf::NowPlaying,
    LyricsSnapshot => jf::LyricsSnapshot,
    ThemeSnapshot => jf::ThemeSnapshot,
    Settings => jf::Settings,
    DevicesSnapshot => jf::DevicesSnapshot,
    SourcesSnapshot => jf::SourcesSnapshot,
    RemoteSnapshot => jf::RemoteSnapshot,
    SearchUpdate => jf::SearchUpdate,
    BrowseUpdate => jf::BrowseUpdate,
);

impl From<jf::Snapshot> for Snapshot {
    fn from(value: jf::Snapshot) -> Self {
        Snapshot {
            playback: value.playback.conv(),
            queue: value.queue.conv(),
            now_playing: value.now_playing.conv(),
            lyrics: value.lyrics.conv(),
            theme: value.theme.conv(),
            library_revision: value.library_revision,
            scanning: value.scanning,
            settings: value.settings.conv(),
            devices: value.devices.conv(),
            sources: value.sources.conv(),
            remote: value.remote.conv(),
            muted: value.muted,
        }
    }
}

impl From<Snapshot> for jf::Snapshot {
    fn from(value: Snapshot) -> Self {
        jf::Snapshot {
            playback: value.playback.conv(),
            queue: value.queue.conv(),
            now_playing: value.now_playing.conv(),
            lyrics: value.lyrics.conv(),
            theme: value.theme.conv(),
            library_revision: value.library_revision,
            scanning: value.scanning,
            settings: value.settings.conv(),
            devices: value.devices.conv(),
            sources: value.sources.conv(),
            remote: value.remote.conv(),
            muted: value.muted,
        }
    }
}

impl From<jf::Event> for Event {
    fn from(value: jf::Event) -> Self {
        match value {
            jf::Event::Playback(snapshot) => Event::Playback {
                snapshot: snapshot.conv(),
            },
            jf::Event::Queue(snapshot) => Event::Queue {
                snapshot: snapshot.conv(),
            },
            jf::Event::NowPlaying(snapshot) => Event::NowPlaying {
                snapshot: snapshot.conv(),
            },
            jf::Event::Lyrics(snapshot) => Event::Lyrics {
                snapshot: snapshot.conv(),
            },
            jf::Event::Theme(snapshot) => Event::Theme {
                snapshot: snapshot.conv(),
            },
            jf::Event::Scan(status) => Event::Scan {
                status: status.into(),
            },
            jf::Event::LibraryChanged { revision } => Event::LibraryChanged { revision },
            jf::Event::PlaylistCreated { request, id } => Event::PlaylistCreated {
                request: request.0,
                id,
            },
            jf::Event::Settings(snapshot) => Event::Settings {
                snapshot: snapshot.conv(),
            },
            jf::Event::Devices(snapshot) => Event::Devices {
                snapshot: snapshot.conv(),
            },
            jf::Event::Sources(snapshot) => Event::Sources {
                snapshot: snapshot.conv(),
            },
            jf::Event::Remote(snapshot) => Event::Remote {
                snapshot: snapshot.conv(),
            },
            jf::Event::Auth(update) => Event::Auth {
                update: update.conv(),
            },
            jf::Event::SearchResults(update) => Event::SearchResults {
                update: update.conv(),
            },
            jf::Event::BrowsePage(update) => Event::BrowsePage {
                update: update.conv(),
            },
            jf::Event::Buffering { uid, buffering } => Event::Buffering { uid, buffering },
            jf::Event::CommandFailed { id, error } => Event::CommandFailed { id: id.0, error },
            jf::Event::Error { message } => Event::Error { message },
        }
    }
}

impl From<Event> for jf::Event {
    fn from(value: Event) -> Self {
        match value {
            Event::Playback { snapshot } => jf::Event::Playback(snapshot.conv()),
            Event::Queue { snapshot } => jf::Event::Queue(snapshot.conv()),
            Event::NowPlaying { snapshot } => jf::Event::NowPlaying(snapshot.conv()),
            Event::Lyrics { snapshot } => jf::Event::Lyrics(snapshot.conv()),
            Event::Theme { snapshot } => jf::Event::Theme(snapshot.conv()),
            Event::Scan { status } => jf::Event::Scan(status.into()),
            Event::LibraryChanged { revision } => jf::Event::LibraryChanged { revision },
            Event::PlaylistCreated { request, id } => jf::Event::PlaylistCreated {
                request: jf::CommandId(request),
                id,
            },
            Event::Settings { snapshot } => jf::Event::Settings(snapshot.conv()),
            Event::Devices { snapshot } => jf::Event::Devices(snapshot.conv()),
            Event::Sources { snapshot } => jf::Event::Sources(snapshot.conv()),
            Event::Remote { snapshot } => jf::Event::Remote(snapshot.conv()),
            Event::Auth { update } => jf::Event::Auth(update.conv()),
            Event::SearchResults { update } => jf::Event::SearchResults(update.conv()),
            Event::BrowsePage { update } => jf::Event::BrowsePage(update.conv()),
            Event::Buffering { uid, buffering } => jf::Event::Buffering { uid, buffering },
            Event::CommandFailed { id, error } => jf::Event::CommandFailed {
                id: jf::CommandId(id),
                error,
            },
            Event::Error { message } => jf::Event::Error { message },
        }
    }
}
