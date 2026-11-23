// Mirrors of jf-ffi's types (core/jf-ffi/src/types.rs), carried as JSON over the C ABI.
// Hand-written: keep in step with types.rs. tests/contract/ffi-types.json checks every shape.
//
// Names: records and members are the Rust names in PascalCase (the JSON names are snake_case).
// Enums with data are abstract records with one nested record per variant (TrackRef.Library,
// Command.PlayCollection, ...), tagged by "type" in JSON. C# forbids a member named like its
// type, so four fields are called Value: Collection.Tracks.Value, ArtworkRef.Url.Value,
// ScanStatus.Progress.Value and Event.Buffering.Value; and Event.CommandFailed's error is
// Message, since Event.Error is a member of every Event record.

using System.Collections.Generic;
using System.Text.Json.Serialization;

namespace Jellyfesh.Ffi;

// ---- setup ----------------------------------------------------------------------------------

/// <summary>How the core is set up.</summary>
/// <param name="DataDir">Holds library.sqlite and session.sqlite.</param>
/// <param name="CacheDir">Holds artwork and baked backdrops; safe to delete.</param>
/// <param name="Output">Where audio goes; the device opens on first playback.</param>
/// <param name="BufferMs">Output buffer length in milliseconds.</param>
/// <param name="RestoreSession">Restore the saved queue, position and settings on start.</param>
/// <param name="PersistSession">Save the queue, position and settings as they change.</param>
/// <param name="WatchFolders">Watch library folders while Settings.WatchFolders is on.</param>
/// <param name="BundledPlugins">.jfpkg packages the app ships: installed at start when missing,
/// updated when newer, never reinstalled once the person uninstalled one. Null means none.</param>
/// <param name="Distribution">How this build reached the person (ADR-0021 §4): which stores sell
/// Jellyfesh Pro here, and whether the core checks for updates (Direct only).</param>
public sealed record CoreConfig(
    string DataDir,
    string CacheDir,
    OutputKind Output,
    uint BufferMs = 200,
    bool RestoreSession = true,
    bool PersistSession = true,
    bool WatchFolders = true,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    IReadOnlyList<string>? BundledPlugins = null,
    Distribution Distribution = Distribution.Source,
    bool ResumeOnStart = true,
    string? DownloadsDir = null);

/// <summary>How this build of the app reached the person.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<Distribution>))]
public enum Distribution
{
    /// <summary>A download from the project's release page: checks for updates itself.</summary>
    Direct,
    AppStore,
    MicrosoftStore,
    GooglePlay,
    /// <summary>Jellyfesh Web.</summary>
    Web,
    /// <summary>Built from source.</summary>
    Source,
}

/// <summary>A store that sells Jellyfesh Pro.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<Store>))]
public enum Store
{
    AppStore,
    MicrosoftStore,
    GooglePlay,
}

/// <summary>What a store says about Jellyfesh Pro, as the app's store glue verified it
/// (JellyfeshCore.SetStoreEntitlement).</summary>
/// <param name="Product">The store's product id (org.jellyfesh.pro).</param>
/// <param name="Active">Whether the person owns it now (false after a refund).</param>
/// <param name="Transaction">The store's transaction or order id, if it gives one.</param>
public sealed record StoreEntitlement(Store Store, string Product, bool Active, string? Transaction);

/// <summary>Where the engine sends audio.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(OutputKind.Device), "Device")]
[JsonDerivedType(typeof(OutputKind.WavFile), "WavFile")]
[JsonDerivedType(typeof(OutputKind.Null), "Null")]
public abstract record OutputKind
{
    private OutputKind() { }

    /// <summary>An audio device; null follows the system default output.</summary>
    public sealed record Device(string? Name) : OutputKind;

    /// <summary>A 32-bit float WAV file, rendered as fast as possible unless Realtime.</summary>
    public sealed record WavFile(string Path, bool Realtime) : OutputKind;

    /// <summary>Discards the audio; paced like a device when Realtime.</summary>
    public sealed record Null(bool Realtime) : OutputKind;
}

// ---- commands -------------------------------------------------------------------------------

/// <summary>Everything a UI can ask the core to do. Failures come back as Event.CommandFailed.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(Command.PlayTracks), "PlayTracks")]
[JsonDerivedType(typeof(Command.Enqueue), "Enqueue")]
[JsonDerivedType(typeof(Command.RemoveFromQueue), "RemoveFromQueue")]
[JsonDerivedType(typeof(Command.MoveInQueue), "MoveInQueue")]
[JsonDerivedType(typeof(Command.JumpTo), "JumpTo")]
[JsonDerivedType(typeof(Command.ClearQueue), "ClearQueue")]
[JsonDerivedType(typeof(Command.SetShuffle), "SetShuffle")]
[JsonDerivedType(typeof(Command.SetRepeat), "SetRepeat")]
[JsonDerivedType(typeof(Command.Play), "Play")]
[JsonDerivedType(typeof(Command.Pause), "Pause")]
[JsonDerivedType(typeof(Command.TogglePlayPause), "TogglePlayPause")]
[JsonDerivedType(typeof(Command.Stop), "Stop")]
[JsonDerivedType(typeof(Command.Next), "Next")]
[JsonDerivedType(typeof(Command.Previous), "Previous")]
[JsonDerivedType(typeof(Command.Seek), "Seek")]
[JsonDerivedType(typeof(Command.SetVolume), "SetVolume")]
[JsonDerivedType(typeof(Command.SetEq), "SetEq")]
[JsonDerivedType(typeof(Command.SetReplayGain), "SetReplayGain")]
[JsonDerivedType(typeof(Command.AddRoot), "AddRoot")]
[JsonDerivedType(typeof(Command.RemoveRoot), "RemoveRoot")]
[JsonDerivedType(typeof(Command.Rescan), "Rescan")]
[JsonDerivedType(typeof(Command.PlayCollection), "PlayCollection")]
[JsonDerivedType(typeof(Command.EnqueueCollection), "EnqueueCollection")]
[JsonDerivedType(typeof(Command.SetSettings), "SetSettings")]
[JsonDerivedType(typeof(Command.SetMuted), "SetMuted")]
[JsonDerivedType(typeof(Command.SetCrossfade), "SetCrossfade")]
[JsonDerivedType(typeof(Command.SetOutputDevice), "SetOutputDevice")]
[JsonDerivedType(typeof(Command.RefreshDevices), "RefreshDevices")]
[JsonDerivedType(typeof(Command.CreatePlaylist), "CreatePlaylist")]
[JsonDerivedType(typeof(Command.RenamePlaylist), "RenamePlaylist")]
[JsonDerivedType(typeof(Command.DeletePlaylist), "DeletePlaylist")]
[JsonDerivedType(typeof(Command.AddToPlaylist), "AddToPlaylist")]
[JsonDerivedType(typeof(Command.RemoveFromPlaylist), "RemoveFromPlaylist")]
[JsonDerivedType(typeof(Command.MovePlaylistEntry), "MovePlaylistEntry")]
[JsonDerivedType(typeof(Command.ImportPlaylist), "ImportPlaylist")]
[JsonDerivedType(typeof(Command.SetFavorite), "SetFavorite")]
[JsonDerivedType(typeof(Command.InstallPlugin), "InstallPlugin")]
[JsonDerivedType(typeof(Command.UninstallPlugin), "UninstallPlugin")]
[JsonDerivedType(typeof(Command.SetPluginEnabled), "SetPluginEnabled")]
[JsonDerivedType(typeof(Command.SetPluginSettings), "SetPluginSettings")]
[JsonDerivedType(typeof(Command.AcknowledgeUnofficial), "AcknowledgeUnofficial")]
[JsonDerivedType(typeof(Command.BeginLogin), "BeginLogin")]
[JsonDerivedType(typeof(Command.ConnectSource), "ConnectSource")]
[JsonDerivedType(typeof(Command.SubmitLogin), "SubmitLogin")]
[JsonDerivedType(typeof(Command.CancelLogin), "CancelLogin")]
[JsonDerivedType(typeof(Command.Logout), "Logout")]
[JsonDerivedType(typeof(Command.Search), "Search")]
[JsonDerivedType(typeof(Command.Browse), "Browse")]
[JsonDerivedType(typeof(Command.Remote), "Remote")]
[JsonDerivedType(typeof(Command.MakeLyrics), "MakeLyrics")]
[JsonDerivedType(typeof(Command.CancelLyrics), "CancelLyrics")]
[JsonDerivedType(typeof(Command.DeleteMadeLyrics), "DeleteMadeLyrics")]
[JsonDerivedType(typeof(Command.AllowAudioUpload), "AllowAudioUpload")]
[JsonDerivedType(typeof(Command.RevokeAudioUpload), "RevokeAudioUpload")]
[JsonDerivedType(typeof(Command.DownloadSpeechModel), "DownloadSpeechModel")]
[JsonDerivedType(typeof(Command.CancelSpeechModelDownload), "CancelSpeechModelDownload")]
[JsonDerivedType(typeof(Command.DeleteSpeechModel), "DeleteSpeechModel")]
[JsonDerivedType(typeof(Command.ResetPluginSettings), "ResetPluginSettings")]
[JsonDerivedType(typeof(Command.ActivateLicence), "ActivateLicence")]
[JsonDerivedType(typeof(Command.RemoveLicence), "RemoveLicence")]
[JsonDerivedType(typeof(Command.CheckForUpdates), "CheckForUpdates")]
[JsonDerivedType(typeof(Command.Discover), "Discover")]
[JsonDerivedType(typeof(Command.Details), "Details")]
[JsonDerivedType(typeof(Command.Comments), "Comments")]
[JsonDerivedType(typeof(Command.SetNetworkState), "SetNetworkState")]
[JsonDerivedType(typeof(Command.SearchMore), "SearchMore")]
[JsonDerivedType(typeof(Command.RefreshSourceLibrary), "RefreshSourceLibrary")]
[JsonDerivedType(typeof(Command.SourceLibraryMore), "SourceLibraryMore")]
[JsonDerivedType(typeof(Command.SavePlaylistCopy), "SavePlaylistCopy")]
[JsonDerivedType(typeof(Command.Download), "Download")]
[JsonDerivedType(typeof(Command.CancelDownload), "CancelDownload")]
[JsonDerivedType(typeof(Command.RetryDownload), "RetryDownload")]
[JsonDerivedType(typeof(Command.RemoveDownload), "RemoveDownload")]
[JsonDerivedType(typeof(Command.RemoveAllDownloads), "RemoveAllDownloads")]
public abstract record Command
{
    private Command() { }

    /// <summary>Replace the queue with Tracks and play the one at Start.</summary>
    public sealed record PlayTracks(IReadOnlyList<TrackRef> Tracks, uint Start) : Command;

    /// <summary>Add to the queue: after the current entry (Play Next) or at the end (Play Last).</summary>
    public sealed record Enqueue(IReadOnlyList<TrackRef> Tracks, bool PlayNext) : Command;

    public sealed record RemoveFromQueue(ulong Uid) : Command;

    public sealed record MoveInQueue(ulong Uid, uint To) : Command;

    /// <summary>Play the queue entry with this uid.</summary>
    public sealed record JumpTo(ulong Uid) : Command;

    public sealed record ClearQueue : Command;

    public sealed record SetShuffle(bool On) : Command;

    public sealed record SetRepeat(RepeatMode Mode) : Command;

    /// <summary>Resume, or start the current entry when stopped.</summary>
    public sealed record Play : Command;

    public sealed record Pause : Command;

    public sealed record TogglePlayPause : Command;

    public sealed record Stop : Command;

    public sealed record Next : Command;

    /// <summary>Restarts the track after its first three seconds, else goes back one entry.</summary>
    public sealed record Previous : Command;

    public sealed record Seek(ulong PositionMs) : Command;

    /// <summary>Linear volume, 0.0 to 1.0.</summary>
    public sealed record SetVolume(float Volume) : Command;

    public sealed record SetEq(EqSettings Eq) : Command;

    public sealed record SetReplayGain(ReplayGainMode Mode, float PreampDb) : Command;

    /// <summary>Add a library folder; the core scans it.</summary>
    public sealed record AddRoot(string Path) : Command;

    public sealed record RemoveRoot(string Path) : Command;

    /// <summary>Scan every library folder for changes.</summary>
    public sealed record Rescan : Command;

    /// <summary>Replace the queue with a collection in its natural order and play from Start.</summary>
    public sealed record PlayCollection(Collection Collection, uint Start, bool Shuffle) : Command;

    public sealed record EnqueueCollection(Collection Collection, bool PlayNext) : Command;

    /// <summary>Replace all settings: send back a modified copy of the Settings slice.</summary>
    public sealed record SetSettings(Settings Settings) : Command;

    public sealed record SetMuted(bool Muted) : Command;

    public sealed record SetCrossfade(CrossfadeSettings Crossfade) : Command;

    /// <summary>Output device by id (or name) from the Devices slice; null follows the system default.</summary>
    public sealed record SetOutputDevice(string? Name) : Command;

    /// <summary>List output devices again (send on OS device notifications).</summary>
    public sealed record RefreshDevices : Command;

    /// <summary>A new playlist, optionally with tracks; Event.PlaylistCreated names it.</summary>
    public sealed record CreatePlaylist(string Name, IReadOnlyList<TrackRef> Tracks) : Command;

    public sealed record RenamePlaylist(long Id, string Name) : Command;

    public sealed record DeletePlaylist(long Id) : Command;

    public sealed record AddToPlaylist(long Id, IReadOnlyList<TrackRef> Tracks) : Command;

    public sealed record RemoveFromPlaylist(long Id, IReadOnlyList<long> Entries) : Command;

    public sealed record MovePlaylistEntry(long Id, long Entry, uint To) : Command;

    /// <summary>Import an M3U/M3U8 file as a new playlist (export is JellyfeshCore.ExportPlaylist).</summary>
    public sealed record ImportPlaylist(string Path) : Command;

    public sealed record SetFavorite(TrackRef Track, bool Favorite) : Command;

    public sealed record InstallPlugin(string Path) : Command;

    public sealed record UninstallPlugin(string Id) : Command;

    public sealed record SetPluginEnabled(string Id, bool Enabled) : Command;

    public sealed record SetPluginSettings(string Id, string SettingsJson) : Command;

    public sealed record AcknowledgeUnofficial(string Id) : Command;

    public sealed record BeginLogin(string Plugin, AuthMethod Method) : Command;

    /// <summary>Connect to a service from SourcesSnapshot.Presets: sets the server's address when the
    /// service asks for one, turns its plugin on and starts signing in, as BeginLogin does.</summary>
    public sealed record ConnectSource(string Preset, string? Address, AuthMethod Method) : Command;

    /// <summary>Answer an AuthStep.AskFields step: field name to value.</summary>
    public sealed record SubmitLogin(string Plugin, ulong Flow, IReadOnlyDictionary<string, string> Fields) : Command;

    public sealed record CancelLogin(string Plugin, ulong Flow) : Command;

    public sealed record Logout(string Plugin) : Command;

    /// <summary>Search the library and sources; results arrive as Event.SearchResults.</summary>
    public sealed record Search(string Query, SearchScope Scope) : Command;

    /// <summary>Browse a source's catalogue: a null Node is its root.</summary>
    public sealed record Browse(string Plugin, string? Node, uint Page) : Command;

    /// <summary>
    /// The remote session (ADR-0018): what the transport commands cannot say. While a session
    /// runs, Play, Pause, TogglePlayPause, Stop, Next, Previous, Seek, SetShuffle and SetRepeat
    /// control the service's player instead of the engine.
    /// </summary>
    public sealed record Remote(RemoteAction Action) : Command;

    /// <summary>Makes synced lyrics for a song with the AI Lyrics plugin (a job in the AiLyrics slice).</summary>
    public sealed record MakeLyrics(SongRef Song) : Command;

    /// <summary>Stops a waiting or running lyrics job.</summary>
    public sealed record CancelLyrics(ulong Job) : Command;

    /// <summary>Deletes the lyrics AI Lyrics made for a song.</summary>
    public sealed record DeleteMadeLyrics(SongRef Song) : Command;

    /// <summary>The person allowed sending songs' audio to Origin for this plugin.</summary>
    public sealed record AllowAudioUpload(string Plugin, string Origin) : Command;

    /// <summary>Stop Allowing: no more audio goes to Origin for this plugin.</summary>
    public sealed record RevokeAudioUpload(string Plugin, string Origin) : Command;

    /// <summary>Downloads an on-device speech model.</summary>
    public sealed record DownloadSpeechModel(string Id) : Command;

    /// <summary>Stops a model download (what came so far is kept).</summary>
    public sealed record CancelSpeechModelDownload(string Id) : Command;

    public sealed record DeleteSpeechModel(string Id) : Command;

    /// <summary>Restores a plugin's settings to the schema's defaults (Reset to Defaults).</summary>
    public sealed record ResetPluginSettings(string Plugin) : Command;

    /// <summary>Activates Jellyfesh Pro with a licence key (JFP1-…).</summary>
    public sealed record ActivateLicence(string Key) : Command;

    public sealed record RemoveLicence : Command;

    /// <summary>Checks for a newer release now (direct downloads only).</summary>
    public sealed record CheckForUpdates : Command;

    /// <summary>A source's Discover page ("library" for the library's); answered by Event.Discover.</summary>
    public sealed record Discover(string Source) : Command;

    /// <summary>More about an item; answered by Event.Details.</summary>
    public sealed record Details(ItemRef Item) : Command;

    /// <summary>A page of comments (null Page is the first); answered by Event.Comments.</summary>
    public sealed record Comments(ItemRef Item, string? Page) : Command;

    /// <summary>The network the platform reports (at start and on every change): Auto quality
    /// asks High while metered; downloads wait offline, and while metered with Wi-Fi Only on.</summary>
    public sealed record SetNetworkState(bool Online, bool Metered) : Command;

    /// <summary>Show More: more of one kind of a source's results for the newest search
    /// (Cursor: the group's Next); answered by Event.SearchPage.</summary>
    public sealed record SearchMore(string Plugin, ItemKind Kind, string Cursor) : Command;

    /// <summary>Reloads a source's likes and playlists (null: every source), when Favourites or
    /// Playlists opens or is returned to after ten minutes.</summary>
    public sealed record RefreshSourceLibrary(string? Plugin) : Command;

    /// <summary>The next page of a source's list; answered by Event.SourceLibraryPage.</summary>
    public sealed record SourceLibraryMore(string Plugin, SourceLibraryPart Part, string Cursor) : Command;

    /// <summary>Save as Playlist…: a Jellyfesh playlist of a source playlist's songs; answered by
    /// Event.PlaylistCreated.</summary>
    public sealed record SavePlaylistCopy(string Plugin, string Id, string Name) : Command;

    /// <summary>Downloads a song, or every song of an album or playlist.</summary>
    public sealed record Download(DownloadTarget Target) : Command;

    /// <summary>Stops an unfinished download and forgets it.</summary>
    public sealed record CancelDownload(ulong Id) : Command;

    /// <summary>Tries a failed download again.</summary>
    public sealed record RetryDownload(ulong Id) : Command;

    /// <summary>Remove Download…: deletes a finished download's file (ask first).</summary>
    public sealed record RemoveDownload(ulong Id) : Command;

    /// <summary>Remove All Downloads… (ask first).</summary>
    public sealed record RemoveAllDownloads : Command;
}

/// <summary>What Command.Download downloads.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(DownloadTarget.Track), "Track")]
[JsonDerivedType(typeof(DownloadTarget.Collection), "Collection")]
public abstract record DownloadTarget
{
    private DownloadTarget() { }

    /// <summary>One song: a TrackRef.Source (the Rust field is `track`).</summary>
    public sealed record Track([property: JsonPropertyName("track")] TrackRef Value) : DownloadTarget;

    /// <summary>Every song of an album or playlist a source listed.</summary>
    public sealed record Collection(string Plugin, string Id, ItemKind ItemKind) : DownloadTarget;
}

/// <summary>A song to make lyrics for.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(SongRef.Current), "Current")]
[JsonDerivedType(typeof(SongRef.Library), "Library")]
public abstract record SongRef
{
    private SongRef() { }

    /// <summary>The song playing now.</summary>
    public sealed record Current : SongRef;

    /// <summary>A library song.</summary>
    public sealed record Library(long Id) : SongRef;
}

/// <summary>An item to show details or comments for.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(ItemRef.Track), "Track")]
[JsonDerivedType(typeof(ItemRef.Album), "Album")]
[JsonDerivedType(typeof(ItemRef.Artist), "Artist")]
[JsonDerivedType(typeof(ItemRef.File), "File")]
[JsonDerivedType(typeof(ItemRef.Source), "Source")]
public abstract record ItemRef
{
    private ItemRef() { }

    /// <summary>A library track.</summary>
    public sealed record Track(long Id) : ItemRef;

    /// <summary>A library album.</summary>
    public sealed record Album(long Id) : ItemRef;

    /// <summary>A library artist.</summary>
    public sealed record Artist(long Id) : ItemRef;

    /// <summary>An audio file that is not in the library.</summary>
    public sealed record File(string Path) : ItemRef;

    /// <summary>An item a source listed: its SourceItem.Source, Kind and Id.</summary>
    public sealed record Source(string Plugin, ItemKind ItemKind, string Id) : ItemRef;
}

/// <summary>What Command.Remote asks of the remote session.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(RemoteAction.Refresh), "Refresh")]
[JsonDerivedType(typeof(RemoteAction.Transfer), "Transfer")]
[JsonDerivedType(typeof(RemoteAction.SetVolume), "SetVolume")]
[JsonDerivedType(typeof(RemoteAction.End), "End")]
public abstract record RemoteAction
{
    private RemoteAction() { }

    /// <summary>Asks the service for its devices and state now (when the device picker opens).</summary>
    public sealed record Refresh : RemoteAction;

    /// <summary>Moves playback to the device with this id, playing there.</summary>
    public sealed record Transfer(string Device) : RemoteAction;

    /// <summary>The playing device's volume, 0–100.</summary>
    public sealed record SetVolume(uint Percent) : RemoteAction;

    /// <summary>Stops controlling the service; its player carries on as it was.</summary>
    public sealed record End : RemoteAction;
}

/// <summary>What a queue entry plays.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(TrackRef.Library), "Library")]
[JsonDerivedType(typeof(TrackRef.File), "File")]
[JsonDerivedType(typeof(TrackRef.Source), "Source")]
public abstract record TrackRef
{
    private TrackRef() { }

    /// <summary>A track in the library, by its library id.</summary>
    public sealed record Library(long Id) : TrackRef;

    /// <summary>A file played directly, outside the library.</summary>
    public sealed record File(string Path) : TrackRef;

    /// <summary>A track from an online source (a plugin).</summary>
    public sealed record Source(SourceTrack Track) : TrackRef;
}

/// <summary>A track from an online source, with the display data the source gave.</summary>
public sealed record SourceTrack(
    string Plugin,
    string Id,
    string Title,
    string? Artist,
    string? Album,
    ulong? DurationMs,
    string? ArtworkUrl,
    string? Isrc);

/// <summary>A group of tracks that plays as a unit.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(Collection.Album), "Album")]
[JsonDerivedType(typeof(Collection.Artist), "Artist")]
[JsonDerivedType(typeof(Collection.Playlist), "Playlist")]
[JsonDerivedType(typeof(Collection.Favorites), "Favorites")]
[JsonDerivedType(typeof(Collection.Songs), "Songs")]
[JsonDerivedType(typeof(Collection.Tracks), "Tracks")]
[JsonDerivedType(typeof(Collection.Source), "Source")]
public abstract record Collection
{
    private Collection() { }

    public sealed record Album(long Id) : Collection;

    public sealed record Artist(long Id) : Collection;

    public sealed record Playlist(long Id) : Collection;

    public sealed record Favorites : Collection;

    /// <summary>Every song in the library, in the Songs page's order.</summary>
    public sealed record Songs(TrackSort Sort, bool Descending) : Collection;

    /// <summary>A list of tracks (the Rust field is `tracks`).</summary>
    public sealed record Tracks([property: JsonPropertyName("tracks")] IReadOnlyList<TrackRef> Value) : Collection;

    /// <summary>An album, playlist or other container from an online source: a SourceItem's
    /// Source, Id and Kind.</summary>
    public sealed record Source(string Plugin, string Id, ItemKind ItemKind) : Collection;
}

/// <summary>Where a search looks.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(SearchScope.All), "All")]
[JsonDerivedType(typeof(SearchScope.Library), "Library")]
[JsonDerivedType(typeof(SearchScope.Plugin), "Plugin")]
public abstract record SearchScope
{
    private SearchScope() { }

    /// <summary>The library and every enabled source.</summary>
    public sealed record All : SearchScope;

    public sealed record Library : SearchScope;

    public sealed record Plugin(string Id) : SearchScope;
}

/// <summary>How a source signs in.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<AuthMethod>))]
public enum AuthMethod
{
    Qr,
    Oauth,
    CookieImport,
    PasswordExchange,
    ApiKey,
}

/// <summary>Repeat mode of the queue.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<RepeatMode>))]
public enum RepeatMode
{
    Off,
    All,
    One,
}

// ---- settings -------------------------------------------------------------------------------

/// <summary>Crossfade between tracks; with album continuity, an album's tracks join gaplessly.</summary>
public sealed record CrossfadeSettings(bool Enabled, uint DurationMs, bool AlbumContinuity);

[JsonConverter(typeof(JsonStringEnumConverter<AppearancePref>))]
public enum AppearancePref
{
    System,
    Light,
    Dark,
}

[JsonConverter(typeof(JsonStringEnumConverter<BackdropStyle>))]
public enum BackdropStyle
{
    Artwork,
    Misty,
}

/// <summary>The size of lyrics on Now Playing.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<LyricsSize>))]
public enum LyricsSize
{
    Small,
    Medium,
    Large,
}

/// <summary>How lyrics lines line up on Now Playing.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<LyricsAlign>))]
public enum LyricsAlign
{
    /// <summary>The start edge (left in left-to-right text).</summary>
    Start,
    Center,
}

/// <summary>Where the theme hue comes from.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(HueSource.Artwork), "Artwork")]
[JsonDerivedType(typeof(HueSource.Fixed), "Fixed")]
public abstract record HueSource
{
    private HueSource() { }

    /// <summary>The current track's artwork (the fallback hue when there is none).</summary>
    public sealed record Artwork : HueSource;

    /// <summary>One fixed hue in degrees.</summary>
    public sealed record Fixed(float Hue) : HueSource;
}

/// <summary>Everything the person sets in Preferences.</summary>
public sealed record Settings(
    AppearancePref Appearance,
    BackdropStyle Style,
    string AppIcon,
    HueSource HueSource,
    bool ReduceEffects,
    float Transparency,
    LyricsSize LyricsSize,
    LyricsAlign LyricsAlign,
    bool ShowTranslation,
    bool ShowRomanization,
    CrossfadeSettings Crossfade,
    ReplayGainMode ReplaygainMode,
    float PreampDb,
    EqSettings Eq,
    string? OutputDevice,
    bool ResumeOnLaunch,
    bool WatchFolders,
    bool AllowCommunityPlugins,
    bool CheckForUpdates = true,
    bool ShowLyricsBar = true,
    bool DesktopLyrics = false,
    StreamingQuality StreamingQuality = StreamingQuality.Auto,
    DownloadSettings? Downloads = null,
    MiniPlayerSettings? MiniPlayer = null);

/// <summary>The streaming level asked of sources (ADR-0026 §1).</summary>
[JsonConverter(typeof(JsonStringEnumConverter<StreamingQuality>))]
public enum StreamingQuality
{
    /// <summary>Lossless, or High on a connection the platform reports as metered.</summary>
    Auto,
    Lossless,
    High,
    Normal,
    Low,
}

/// <summary>The quality downloads are asked for (ADR-0025 §2).</summary>
[JsonConverter(typeof(JsonStringEnumConverter<DownloadQuality>))]
public enum DownloadQuality
{
    /// <summary>The file exactly as the server keeps it.</summary>
    Original,
    High,
    Normal,
    Low,
}

/// <summary>Settings › Downloads (ADR-0025 §2). Null Folder uses the default
/// (DownloadsSnapshot.Folder); DownloadPreview shows what NameTemplate makes.</summary>
public sealed record DownloadSettings(
    string? Folder = null,
    DownloadQuality Quality = DownloadQuality.Original,
    string NameTemplate = "{album_artist}/{album}/{track} {title}",
    bool AddMissingCoverAndLyrics = false,
    bool LyricsInTags = true,
    bool SaveLrcFiles = false,
    bool WifiOnly = true,
    uint AtOnce = 2);

/// <summary>Settings › Appearance › Mini Player (ADR-0026 §4).</summary>
public sealed record MiniPlayerSettings(
    bool KeepOnTop = true,
    bool OpenWhenWindowClosed = false,
    bool ShowLyricsLine = true,
    bool AndroidPip = false);

[JsonConverter(typeof(JsonStringEnumConverter<ReplayGainMode>))]
public enum ReplayGainMode
{
    Off,
    Track,
    Album,
}

/// <summary>Equalizer settings: a preamp and up to ten bands.</summary>
public sealed record EqSettings(bool Enabled, float PreampDb, IReadOnlyList<EqBand> Bands);

public sealed record EqBand(BandKind Kind, float FreqHz, float GainDb, float Q, bool Enabled);

/// <summary>An equalizer preset (JellyfeshMethods.EqPresets). Id is flat, bass_boost, vocal or treble: show it
/// translated ("Flat", "Bass Boost", "Vocal", "Treble").</summary>
public sealed record EqPreset(string Id, EqSettings Settings);

[JsonConverter(typeof(JsonStringEnumConverter<BandKind>))]
public enum BandKind
{
    Peaking,
    LowShelf,
    HighShelf,
}

// ---- devices and sources --------------------------------------------------------------------

/// <summary>A sound output device. Id is stable and unique; OutputKind.Device and SetOutputDevice accept it.</summary>
public sealed record DeviceInfo(string Id, string Name, bool IsDefault, IReadOnlyList<uint> SampleRates);

/// <summary>The Devices slice. A null Selected follows the system default.</summary>
public sealed record DevicesSnapshot(ulong Version, IReadOnlyList<DeviceInfo> Devices, string? Selected);

[JsonConverter(typeof(JsonStringEnumConverter<PluginBadge>))]
public enum PluginBadge
{
    Official,
    Community,
}

/// <summary>One installed source.</summary>
/// <param name="Description">One line from the manifest, if it has one.</param>
/// <param name="SigningKey">Community packages: the fingerprint of the key that signed it.</param>
/// <param name="DisabledReason">Why it cannot be used, when the host turned it off or community
/// plugins are off.</param>
/// <param name="Attribution">Attribution its views must show, if any.</param>
/// <param name="Icon">The package's icon as a PNG file in the cache folder.</param>
/// <param name="RemotePlayback">Remote playback: the service plays on the person's own devices
/// and Jellyfesh is its remote control; its tracks start a remote session and never join the
/// queue.</param>
public sealed record PluginSummary(
    string Id,
    string Name,
    string Version,
    string Author,
    string? Description,
    PluginBadge Badge,
    string? SigningKey,
    bool VendorPlayback,
    IReadOnlyList<string> Capabilities,
    IReadOnlyList<AuthMethod> AuthMethods,
    bool Enabled,
    string? DisabledReason,
    string? Account,
    bool NeedsDisclosure,
    string? Attribution,
    string SettingsSchemaJson,
    string SettingsJson,
    string? Icon,
    bool RemotePlayback = false,
    PluginKind Kind = PluginKind.Source,
    bool RequiresPro = false,
    IReadOnlyList<string>? Permissions = null,
    bool Discover = false,
    bool Details = false,
    bool Comments = false,
    bool Library = false,
    bool Download = false,
    IReadOnlyList<StreamingQuality>? Qualities = null,
    bool LocalNetwork = false);

/// <summary>Where Settings lists a plugin: Sources, or Extensions (AI Lyrics, Customize).</summary>
[JsonConverter(typeof(JsonStringEnumConverter<PluginKind>))]
public enum PluginKind
{
    Source,
    Extension,
}

/// <summary>Where sign-ins are kept.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<SecretStorage>))]
public enum SecretStorage
{
    /// <summary>The system keystore, or the store the app passed in.</summary>
    Keystore,
    /// <summary>No keystore could be reached: sign-ins last until the app quits.</summary>
    SessionOnly,
}

/// <summary>A music service the apps offer in Sources → Add a Source, listed while its official
/// plugin is installed. Command.ConnectSource connects to it.</summary>
/// <param name="Id">Stable name: jellyfin, navidrome, subsonic.</param>
/// <param name="NeedsAddress">Whether the person types the server's address (a self-hosted server).</param>
/// <param name="Address">The address set now, to fill the field with.</param>
/// <param name="AddressExample">An address to show as an example in that field.</param>
public sealed record SourcePreset(
    string Id,
    string Name,
    string Plugin,
    IReadOnlyList<AuthMethod> AuthMethods,
    bool NeedsAddress,
    string? Address,
    string? AddressExample,
    string? Account);

/// <summary>The Sources slice.</summary>
/// <param name="Presets">The services offered in Sources → Add a Source, in their order.</param>
/// <param name="SecretStorage">Whether sign-ins outlive this run.</param>
/// <param name="Unavailable">Set when online sources cannot run at all.</param>
public sealed record SourcesSnapshot(
    ulong Version,
    IReadOnlyList<PluginSummary> Plugins,
    IReadOnlyList<SourcePreset> Presets,
    bool AllowCommunity,
    SecretStorage SecretStorage,
    string? Unavailable,
    bool CanInstallPlugins = true);

/// <summary>How a sign-in field is entered.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<AuthFieldKind>))]
public enum AuthFieldKind
{
    Text,
    Email,
    Url,
    Number,
    CookieHeader,
}

public sealed record AuthField(
    string Name,
    string Label,
    AuthFieldKind Kind,
    bool Secret,
    bool Required,
    string? Placeholder,
    string? Help);

/// <summary>One step of a sign-in flow.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(AuthStep.ShowQr), "ShowQr")]
[JsonDerivedType(typeof(AuthStep.ShowCode), "ShowCode")]
[JsonDerivedType(typeof(AuthStep.OpenUrl), "OpenUrl")]
[JsonDerivedType(typeof(AuthStep.AskFields), "AskFields")]
[JsonDerivedType(typeof(AuthStep.Waiting), "Waiting")]
[JsonDerivedType(typeof(AuthStep.Done), "Done")]
[JsonDerivedType(typeof(AuthStep.Failed), "Failed")]
public abstract record AuthStep
{
    private AuthStep() { }

    /// <summary>Show Data (usually a URL) as a QR code; the core polls meanwhile.</summary>
    public sealed record ShowQr(string Data, string? Hint, long? ExpiresAtMs) : AuthStep;

    /// <summary>Show Code large, to be entered on another device (Hint says where).</summary>
    public sealed record ShowCode(string Code, string Hint, long? ExpiresAtMs) : AuthStep;

    public sealed record OpenUrl(string Url) : AuthStep;

    public sealed record AskFields(IReadOnlyList<AuthField> Fields) : AuthStep;

    public sealed record Waiting(string Message) : AuthStep;

    public sealed record Done(string Account) : AuthStep;

    public sealed record Failed(string Message) : AuthStep;
}

public sealed record AuthUpdate(string Plugin, ulong Flow, AuthStep Step);

// ---- search, browse and lists ---------------------------------------------------------------

[JsonConverter(typeof(JsonStringEnumConverter<ItemKind>))]
public enum ItemKind
{
    Track,
    Album,
    Artist,
    Playlist,
    Folder,
}

/// <summary>Artwork a UI can load: a local file or a URL.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(ArtworkRef.File), "File")]
[JsonDerivedType(typeof(ArtworkRef.Url), "Url")]
public abstract record ArtworkRef
{
    private ArtworkRef() { }

    public sealed record File(string Path) : ArtworkRef;

    /// <summary>A URL (the Rust field is `url`).</summary>
    public sealed record Url([property: JsonPropertyName("url")] string Value) : ArtworkRef;
}

/// <summary>A search or browse result from the library or a source.</summary>
/// <param name="Link">An https link that opens the item in its service ("Open in {Service}"),
/// when its source declares one; the main action for items that cannot play here.</param>
public sealed record SourceItem(
    string Source,
    ItemKind Kind,
    string Id,
    string Title,
    string? Subtitle,
    string? Album,
    ulong? DurationMs,
    ArtworkRef? Artwork,
    bool Playable,
    string? UnplayableReason,
    TrackRef? Track,
    IReadOnlyList<string> AlsoIn,
    string? Link = null,
    string? ArtistId = null,
    string? AlbumId = null,
    bool Downloaded = false);

/// <summary>Results from one source for one search; Done on the last.</summary>
/// <param name="Items">The section for the All tab: with Merged, less what earlier sections show;
/// otherwise all its results.</param>
/// <param name="Merged">Part of the merged results; false for sources whose terms forbid merging
/// (never de-duplicated, never shown beside other rows: the All tab links to their tab).</param>
/// <param name="Groups">The source's own results kind by kind, for its tab, with Show More.</param>
public sealed record SearchUpdate(
    ulong Request,
    string Query,
    string Source,
    IReadOnlyList<SourceItem> Items,
    string? Error,
    bool Done,
    bool Merged = true,
    IReadOnlyList<SearchGroup>? Groups = null);

/// <summary>One kind of a source's search results; Next is Show More's cursor.</summary>
public sealed record SearchGroup(ItemKind Kind, IReadOnlyList<SourceItem> Items, string? Next, uint? Total);

/// <summary>More of one kind of a source's results (Command.SearchMore).</summary>
public sealed record SearchPage(
    ulong Request,
    string Query,
    string Source,
    ItemKind Kind,
    IReadOnlyList<SourceItem> Items,
    string? Next,
    string? Error);

/// <summary>One page of a source's catalogue.</summary>
public sealed record BrowseUpdate(
    ulong Request,
    string Plugin,
    string? Node,
    IReadOnlyList<SourceItem> Items,
    uint? NextPage,
    string? Error);

/// <summary>A row in a track list: what to show, and the TrackRef to queue.</summary>
/// <param name="Artwork">A source track's image URL; library rows have ArtworkHash.</param>
/// <param name="ArtworkHash">Library rows: the artwork's hash, for ArtworkThumbnail.</param>
/// <param name="Missing">The file is gone; the row cannot play.</param>
public sealed record TrackRow(
    TrackRef Track,
    string Title,
    string? Artist,
    string? Album,
    ulong? DurationMs,
    ArtworkRef? Artwork,
    string? ArtworkHash,
    bool Favorite,
    bool Missing,
    bool Downloaded = false);

public sealed record PlaylistSummary(
    long Id,
    string Name,
    uint EntryCount,
    ulong DurationMs,
    long UpdatedAtMs,
    string? ArtworkHash);

public sealed record PlaylistEntry(long EntryId, uint Position, TrackRow Row);

// ---- library --------------------------------------------------------------------------------

/// <summary>One audio file in the library. Missing tags are null.</summary>
public sealed record Track(
    long Id,
    string Path,
    string Title,
    string? Artist,
    string? Album,
    string? AlbumArtist,
    long? AlbumId,
    uint? TrackNo,
    uint? DiscNo,
    int? Year,
    string? Genre,
    ulong? DurationMs,
    uint? SampleRate,
    byte? Channels,
    byte? BitDepth,
    string? Codec,
    ReplayGain Replaygain,
    string? Isrc,
    bool HasEmbeddedLyrics,
    string? ArtworkHash,
    ulong FileSize,
    long MtimeNs);

public sealed record ReplayGain(float? TrackGainDb, float? TrackPeak, float? AlbumGainDb, float? AlbumPeak);

public sealed record Album(
    long Id,
    string Title,
    string? AlbumArtist,
    int? Year,
    uint TrackCount,
    string? ArtworkHash);

public sealed record Artist(long Id, string Name, uint TrackCount);

[JsonConverter(typeof(JsonStringEnumConverter<TrackSort>))]
public enum TrackSort
{
    Artist,
    Title,
    Album,
    Year,
    Duration,
    Path,
    Added,
}

/// <summary>Sort order for AlbumsSorted; unknown artists and years sort last.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<AlbumSort>))]
public enum AlbumSort
{
    Title,
    Artist,
    Year,
    Added,
}

/// <summary>A page of tracks for Tracks.</summary>
public sealed record TrackQuery(
    uint Offset = 0,
    uint Limit = 100,
    TrackSort Sort = TrackSort.Artist,
    bool Descending = false,
    long? AlbumId = null,
    long? ArtistId = null);

public sealed record SearchResults(IReadOnlyList<Track> Tracks, IReadOnlyList<Album> Albums, IReadOnlyList<Artist> Artists);

public sealed record LibraryStats(
    ulong Roots,
    ulong Tracks,
    ulong Albums,
    ulong Artists,
    ulong Artwork,
    ulong TotalDurationMs,
    ulong TotalBytes);

public sealed record ScanProgress(ulong FilesSeen, ulong FilesChanged, string Current);

public sealed record ScanReport(ulong Added, ulong Updated, ulong Removed, ulong Unchanged, IReadOnlyList<ScanError> Errors);

/// <summary>A file a scan could not read, and why.</summary>
public sealed record ScanError(string Path, string Message);

/// <summary>Library scan progress.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(ScanStatus.Progress), "Progress")]
[JsonDerivedType(typeof(ScanStatus.Finished), "Finished")]
[JsonDerivedType(typeof(ScanStatus.Failed), "Failed")]
public abstract record ScanStatus
{
    private ScanStatus() { }

    /// <summary>Progress (the Rust field is `progress`).</summary>
    public sealed record Progress([property: JsonPropertyName("progress")] ScanProgress Value) : ScanStatus;

    public sealed record Finished(ScanReport Report) : ScanStatus;

    public sealed record Failed(string Message) : ScanStatus;
}

// ---- playback and queue ---------------------------------------------------------------------

[JsonConverter(typeof(JsonStringEnumConverter<Status>))]
public enum Status
{
    Stopped,
    Playing,
    Paused,
    Ended,
}

/// <summary>
/// A point on the playback timeline: position(now) = PositionUs + (now - AnchorMonoNs) * Rate / 1000,
/// on the clock of JellyfeshMethods.MonotonicNowNs(). Feed it to a DisplayClock.
/// </summary>
public sealed record PlaybackAnchor(
    ulong TrackGeneration,
    long PositionUs,
    ulong AnchorMonoNs,
    float Rate,
    long? DurationUs);

/// <summary>The Playback slice.</summary>
public sealed record PlaybackSnapshot(
    ulong Version,
    Status Status,
    PlaybackAnchor Anchor,
    float Volume,
    EqSettings Eq,
    ReplayGainMode ReplaygainMode,
    float PreampDb);

public sealed record QueueItem(ulong Uid, TrackRef Track);

/// <summary>The Queue slice; UpNext holds at most 50 entries (QueueWindow pages the rest).</summary>
public sealed record QueueSnapshot(
    ulong Version,
    uint Len,
    uint? Position,
    QueueItem? Current,
    bool Shuffle,
    RepeatMode Repeat,
    IReadOnlyList<QueueItem> UpNext);

public sealed record HistoryEntry(TrackRef Track, long PlayedAtMs);

/// <summary>Facts about an opened track; Bitrate in bits per second when known.</summary>
public sealed record TrackInfo(string Codec, uint SampleRate, ushort Channels, long? DurationUs, bool Gapless, uint? Bitrate, uint? BitsPerSample = null);

/// <summary>What is playing, with display metadata.</summary>
/// <param name="Link">Opens the track in its service, for source tracks whose plugin declares
/// links.</param>
public sealed record NowPlaying(
    ulong Uid,
    TrackRef Track,
    string? Path,
    string Title,
    string? Artist,
    string? Album,
    ulong? DurationMs,
    ArtworkRef? Artwork,
    TrackInfo? Format,
    string? Link = null,
    AudioFormat? Quality = null,
    IReadOnlyList<StreamingQuality>? Qualities = null,
    QualityNote? QualityNote = null,
    StreamingQuality? QualityLevel = null);

/// <summary>Why the quality menu offers no levels; the apps word it.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(QualityNote.FilesPlayAsTheyAre), "FilesPlayAsTheyAre")]
[JsonDerivedType(typeof(QualityNote.SetInService), "SetInService")]
public abstract record QualityNote
{
    private QualityNote() { }

    /// <summary>"Files play as they are."</summary>
    public sealed record FilesPlayAsTheyAre : QualityNote;

    /// <summary>"Quality is set in {Service}."</summary>
    public sealed record SetInService(string Service) : QualityNote;
}

// ---- remote sessions ------------------------------------------------------------------------

[JsonConverter(typeof(JsonStringEnumConverter<RemoteDeviceKind>))]
public enum RemoteDeviceKind
{
    Computer,
    Smartphone,
    Speaker,
    Tv,
    Car,
    Other,
}

/// <summary>A device a remote source can play on.</summary>
/// <param name="Active">The device playing (or last playing) for this account.</param>
/// <param name="VolumePercent">Its volume, 0–100, when it lets Jellyfesh change it.</param>
public sealed record RemoteDevice(string Id, string Name, RemoteDeviceKind Kind, bool Active, uint? VolumePercent);

/// <summary>
/// The Remote slice: the remote session, while Jellyfesh controls a service's own player. Apps
/// show it as Now Playing instead of the engine's track while Source is set.
/// </summary>
/// <param name="Source">The source in control; null when there is no remote session.</param>
/// <param name="SourceName">That source's name, for "Playing on {device} · {source}".</param>
/// <param name="Item">The track playing, as the service reports it.</param>
/// <param name="Context">The album or playlist it plays from, when the service says.</param>
/// <param name="Anchor">Time base for the progress bar, like the playback slice's.</param>
/// <param name="Device">The device playing.</param>
/// <param name="Devices">Every device the service lists, for the device picker.</param>
/// <param name="Error">What went wrong last talking to the service, until a call succeeds.</param>
public sealed record RemoteSnapshot(
    ulong Version,
    string? Source,
    string? SourceName,
    Status Status,
    SourceItem? Item,
    SourceItem? Context,
    PlaybackAnchor Anchor,
    bool Shuffle,
    RepeatMode Repeat,
    RemoteDevice? Device,
    IReadOnlyList<RemoteDevice> Devices,
    string? Error,
    QualityNote? QualityNote = null);

// ---- lyrics ---------------------------------------------------------------------------------

/// <summary>One lyric document; times are milliseconds with the source offset applied.</summary>
public sealed record LyricDoc(
    string? Language,
    Timing Timing,
    int OffsetMs,
    IReadOnlyList<Agent> Agents,
    IReadOnlyList<Line> Lines,
    IReadOnlyList<AuxTrack> Tracks,
    SourceInfo Source,
    IReadOnlyDictionary<string, string> Metadata);

[JsonConverter(typeof(JsonStringEnumConverter<Timing>))]
public enum Timing
{
    Unsynced,
    Line,
    Word,
}

public sealed record Agent(string Id, string? Name, AgentKind Kind);

[JsonConverter(typeof(JsonStringEnumConverter<AgentKind>))]
public enum AgentKind
{
    Person,
    Group,
    Other,
}

public sealed record Line(
    uint Id,
    uint? StartMs,
    uint? EndMs,
    string? Agent,
    string Text,
    IReadOnlyList<Word> Words,
    IReadOnlyList<Word> Background,
    Direction Direction);

public sealed record Word(uint StartMs, uint EndMs, string Text, bool JoinsNext);

[JsonConverter(typeof(JsonStringEnumConverter<Direction>))]
public enum Direction
{
    Ltr,
    Rtl,
    Auto,
}

public sealed record AuxTrack(AuxKind Kind, string? Language, IReadOnlyDictionary<uint, AuxLine> Lines);

[JsonConverter(typeof(JsonStringEnumConverter<AuxKind>))]
public enum AuxKind
{
    Translation,
    Romanization,
}

public sealed record AuxLine(string Text, IReadOnlyList<Word> Words);

public sealed record SourceInfo(Format Format, string? Origin);

[JsonConverter(typeof(JsonStringEnumConverter<Format>))]
public enum Format
{
    Lrc,
    EnhancedLrc,
    Ttml,
    WebVtt,
    Plain,
}

/// <summary>
/// Flat lookup arrays for the per-frame lyric cursor (see LyricTimeline.Cursor in
/// platforms/shared/csharp). SegmentLines and LineEnds use uint.MaxValue for "no line" and "no end".
/// </summary>
public sealed record LyricTimeline(
    IReadOnlyList<uint> LineStarts,
    IReadOnlyList<uint> LineEnds,
    IReadOnlyList<uint> SegmentStarts,
    IReadOnlyList<uint> SegmentLines,
    IReadOnlyList<uint> WordOffsets,
    IReadOnlyList<uint> WordStarts,
    IReadOnlyList<uint> WordEnds);

/// <summary>Where lyrics came from.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(LyricsSource.Sidecar), "Sidecar")]
[JsonDerivedType(typeof(LyricsSource.Embedded), "Embedded")]
[JsonDerivedType(typeof(LyricsSource.Plugin), "Plugin")]
[JsonDerivedType(typeof(LyricsSource.Made), "Made")]
public abstract record LyricsSource
{
    private LyricsSource() { }

    /// <summary>A .ttml, .lrc or .vtt file next to the audio file.</summary>
    public sealed record Sidecar(string Path) : LyricsSource;

    /// <summary>Lyrics stored in the audio file's tags.</summary>
    public sealed record Embedded : LyricsSource;

    /// <summary>The plugin with this id (its attribution is in LyricDoc.Source.Origin).</summary>
    public sealed record Plugin(string Id) : LyricsSource;

    /// <summary>Made by AI Lyrics: "Made by AI Lyrics · {Engine}". The Rust field `plugin` is
    /// PluginId here, since LyricsSource.Plugin is a sibling record.</summary>
    public sealed record Made([property: JsonPropertyName("plugin")] string PluginId, string Engine) : LyricsSource;
}

/// <summary>The Lyrics slice for the current track.</summary>
public sealed record LyricsSnapshot(ulong Uid, LyricDoc? Doc, LyricTimeline? Timeline, LyricsSource? Source);

// ---- theme ----------------------------------------------------------------------------------

[JsonConverter(typeof(JsonStringEnumConverter<Appearance>))]
public enum Appearance
{
    Light,
    Dark,
}

/// <summary>One theme colour: its token name (ink.ink-90) and 8-bit sRGB with alpha as 0xRRGGBBAA.</summary>
public sealed record ThemeColor(string Name, uint Rgba);

/// <summary>Every theme colour for one hue and appearance, sorted by name.</summary>
public sealed record Theme(float Hue, Appearance Appearance, IReadOnlyList<ThemeColor> Colors);

/// <summary>The Theme slice: Misty colours from the artwork's hue, and baked backdrops.</summary>
public sealed record ThemeSnapshot(
    ulong? Uid,
    float Hue,
    bool FromArtwork,
    Theme Light,
    Theme Dark,
    string? BackdropLight,
    string? BackdropDark);

// ---- snapshot and events --------------------------------------------------------------------

/// <summary>Everything a UI mirrors. Each slice carries its own version.</summary>
public sealed record Snapshot(
    PlaybackSnapshot Playback,
    QueueSnapshot Queue,
    NowPlaying? NowPlaying,
    LyricsSnapshot? Lyrics,
    ThemeSnapshot? Theme,
    ulong LibraryRevision,
    bool Scanning,
    Settings Settings,
    DevicesSnapshot Devices,
    SourcesSnapshot Sources,
    bool Muted,
    RemoteSnapshot? Remote = null,
    AiLyricsSnapshot? AiLyrics = null,
    LookSnapshot? Look = null,
    ProSnapshot? Pro = null,
    UpdateSnapshot? Update = null,
    SourceLibrarySnapshot? SourceLibrary = null,
    DownloadsSnapshot? Downloads = null);

/// <summary>What the core tells the listener. Slice events carry the slice as Snapshot.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(Event.Playback), "Playback")]
[JsonDerivedType(typeof(Event.Queue), "Queue")]
[JsonDerivedType(typeof(Event.NowPlaying), "NowPlaying")]
[JsonDerivedType(typeof(Event.Lyrics), "Lyrics")]
[JsonDerivedType(typeof(Event.Theme), "Theme")]
[JsonDerivedType(typeof(Event.Scan), "Scan")]
[JsonDerivedType(typeof(Event.LibraryChanged), "LibraryChanged")]
[JsonDerivedType(typeof(Event.PlaylistCreated), "PlaylistCreated")]
[JsonDerivedType(typeof(Event.Settings), "Settings")]
[JsonDerivedType(typeof(Event.Devices), "Devices")]
[JsonDerivedType(typeof(Event.Sources), "Sources")]
[JsonDerivedType(typeof(Event.Remote), "Remote")]
[JsonDerivedType(typeof(Event.Auth), "Auth")]
[JsonDerivedType(typeof(Event.SearchResults), "SearchResults")]
[JsonDerivedType(typeof(Event.BrowsePage), "BrowsePage")]
[JsonDerivedType(typeof(Event.Buffering), "Buffering")]
[JsonDerivedType(typeof(Event.CommandFailed), "CommandFailed")]
[JsonDerivedType(typeof(Event.Error), "Error")]
[JsonDerivedType(typeof(Event.AiLyrics), "AiLyrics")]
[JsonDerivedType(typeof(Event.Look), "Look")]
[JsonDerivedType(typeof(Event.Pro), "Pro")]
[JsonDerivedType(typeof(Event.UpdateStatus), "Update")]
[JsonDerivedType(typeof(Event.Discover), "Discover")]
[JsonDerivedType(typeof(Event.Details), "Details")]
[JsonDerivedType(typeof(Event.Comments), "Comments")]
[JsonDerivedType(typeof(Event.Artwork), "Artwork")]
[JsonDerivedType(typeof(Event.SearchPage), "SearchPage")]
[JsonDerivedType(typeof(Event.SourceLibrary), "SourceLibrary")]
[JsonDerivedType(typeof(Event.SourceLibraryPage), "SourceLibraryPage")]
[JsonDerivedType(typeof(Event.Downloads), "Downloads")]
public abstract record Event
{
    private Event() { }

    public sealed record Playback(PlaybackSnapshot Snapshot) : Event;

    public sealed record Queue(QueueSnapshot Snapshot) : Event;

    public sealed record NowPlaying(global::Jellyfesh.Ffi.NowPlaying? Snapshot) : Event;

    public sealed record Lyrics(LyricsSnapshot Snapshot) : Event;

    public sealed record Theme(ThemeSnapshot Snapshot) : Event;

    public sealed record Scan(ScanStatus Status) : Event;

    /// <summary>The catalogue, a playlist, a favourite or a play count changed: query lists again.</summary>
    public sealed record LibraryChanged(ulong Revision) : Event;

    /// <summary>CreatePlaylist or ImportPlaylist (the command Send returned Request for) made playlist Id.</summary>
    public sealed record PlaylistCreated(ulong Request, long Id) : Event;

    public sealed record Settings(global::Jellyfesh.Ffi.Settings Snapshot) : Event;

    public sealed record Devices(DevicesSnapshot Snapshot) : Event;

    public sealed record Sources(SourcesSnapshot Snapshot) : Event;

    /// <summary>The remote session changed.</summary>
    public sealed record Remote(RemoteSnapshot Snapshot) : Event;

    public sealed record Auth(AuthUpdate Update) : Event;

    public sealed record SearchResults(SearchUpdate Update) : Event;

    public sealed record BrowsePage(BrowseUpdate Update) : Event;

    /// <summary>A source is buffering the current track (the Rust field is `buffering`).</summary>
    public sealed record Buffering(ulong Uid, [property: JsonPropertyName("buffering")] bool Value) : Event;

    /// <summary>
    /// The command with this id (as returned by Send) failed. The Rust field is `error`; C# calls
    /// it Message because Event.Error is a member of every Event record.
    /// </summary>
    public sealed record CommandFailed(ulong Id, [property: JsonPropertyName("error")] string Message) : Event;

    /// <summary>A problem not tied to a command.</summary>
    public sealed record Error(string Message) : Event;

    public sealed record AiLyrics(AiLyricsSnapshot Snapshot) : Event;

    public sealed record Look(LookSnapshot Snapshot) : Event;

    public sealed record Pro(ProSnapshot Snapshot) : Event;

    /// <summary>The Update slice changed. The Rust variant is `Update`; C# calls it UpdateStatus
    /// because Update is a member of Event.Auth, SearchResults and BrowsePage.</summary>
    public sealed record UpdateStatus(UpdateSnapshot Snapshot) : Event;

    /// <summary>A Discover page (Command.Discover).</summary>
    public sealed record Discover(DiscoverPage Page) : Event;

    /// <summary>An item's details (Command.Details).</summary>
    public sealed record Details(DetailsPage Page) : Event;

    /// <summary>A page of comments (Command.Comments).</summary>
    public sealed record Comments(CommentsPage Page) : Event;

    /// <summary>A picture asked for with SourceArtwork is cached (Ready.Path), or cannot be had (null).</summary>
    public sealed record Artwork(ArtworkReady Ready) : Event;

    /// <summary>More of one kind of a source's search results (Command.SearchMore).</summary>
    public sealed record SearchPage(global::Jellyfesh.Ffi.SearchPage Page) : Event;

    /// <summary>The SourceLibrary slice changed.</summary>
    public sealed record SourceLibrary(SourceLibrarySnapshot Snapshot) : Event;

    /// <summary>More of a source's list (Command.SourceLibraryMore).</summary>
    public sealed record SourceLibraryPage(global::Jellyfesh.Ffi.SourceLibraryPage Page) : Event;

    /// <summary>The Downloads slice changed.</summary>
    public sealed record Downloads(DownloadsSnapshot Snapshot) : Event;
}

// ---- AI Lyrics (ADR-0019) -------------------------------------------------------------------

[JsonConverter(typeof(JsonStringEnumConverter<AiLyricsMode>))]
public enum AiLyricsMode
{
    /// <summary>Whisper in the app; nothing leaves the device.</summary>
    OnDevice,
    /// <summary>The person's own provider, with their API key, after consent.</summary>
    Online,
}

/// <summary>What making lyrics would do now.</summary>
/// <param name="Engine">The engine, in the plugin's words: "Whisper Base on this device".</param>
/// <param name="UploadTo">The origin the online mode sends audio to.</param>
/// <param name="Automatic">Lyrics are made for each song that starts without synced lyrics.</param>
public sealed record AiLyricsPlan(AiLyricsMode Mode, string Engine, string? UploadTo, bool Automatic);

/// <summary>What must happen before lyrics can be made.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(AiLyricsNeed.Nothing), "Nothing")]
[JsonDerivedType(typeof(AiLyricsNeed.Pro), "Pro")]
[JsonDerivedType(typeof(AiLyricsNeed.SignIn), "SignIn")]
[JsonDerivedType(typeof(AiLyricsNeed.Consent), "Consent")]
[JsonDerivedType(typeof(AiLyricsNeed.Model), "Model")]
[JsonDerivedType(typeof(AiLyricsNeed.Unavailable), "Unavailable")]
public abstract record AiLyricsNeed
{
    private AiLyricsNeed() { }

    /// <summary>Ready: MakeLyrics works.</summary>
    public sealed record Nothing : AiLyricsNeed;

    /// <summary>Jellyfesh Pro is inactive: show the Pro sheet.</summary>
    public sealed record Pro : AiLyricsNeed;

    /// <summary>The online mode needs the person's API key: open AI Lyrics settings.</summary>
    public sealed record SignIn : AiLyricsNeed;

    /// <summary>Ask "Send This Song to {Origin}?" and answer with AllowAudioUpload.</summary>
    public sealed record Consent(string Origin) : AiLyricsNeed;

    /// <summary>Download this model first.</summary>
    public sealed record Model(string Id, string Name, ulong Bytes) : AiLyricsNeed;

    /// <summary>Making lyrics cannot work as set up.</summary>
    public sealed record Unavailable(string Reason) : AiLyricsNeed;
}

/// <summary>Where a lyrics job is.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(LyricsJobStage.Waiting), "Waiting")]
[JsonDerivedType(typeof(LyricsJobStage.ReadingAudio), "ReadingAudio")]
[JsonDerivedType(typeof(LyricsJobStage.Sending), "Sending")]
[JsonDerivedType(typeof(LyricsJobStage.Recognising), "Recognising")]
[JsonDerivedType(typeof(LyricsJobStage.TimingWords), "TimingWords")]
[JsonDerivedType(typeof(LyricsJobStage.Done), "Done")]
[JsonDerivedType(typeof(LyricsJobStage.Failed), "Failed")]
[JsonDerivedType(typeof(LyricsJobStage.Cancelled), "Cancelled")]
public abstract record LyricsJobStage
{
    private LyricsJobStage() { }

    public sealed record Waiting : LyricsJobStage;

    public sealed record ReadingAudio : LyricsJobStage;

    public sealed record Sending(string Origin) : LyricsJobStage;

    /// <summary>Fraction 0–1.</summary>
    public sealed record Recognising(float Fraction) : LyricsJobStage;

    public sealed record TimingWords : LyricsJobStage;

    public sealed record Done : LyricsJobStage;

    public sealed record Failed(string Message) : LyricsJobStage;

    public sealed record Cancelled : LyricsJobStage;
}

/// <summary>A lyrics job: Uid when made for the song playing, LibraryId when the song is in the library.</summary>
public sealed record LyricsJob(ulong Id, string Title, string? Artist, ulong? Uid, long? LibraryId, LyricsJobStage Stage);

/// <summary>Where an on-device speech model is.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(SpeechModelStatus.NotDownloaded), "NotDownloaded")]
[JsonDerivedType(typeof(SpeechModelStatus.Downloading), "Downloading")]
[JsonDerivedType(typeof(SpeechModelStatus.Ready), "Ready")]
[JsonDerivedType(typeof(SpeechModelStatus.Failed), "Failed")]
public abstract record SpeechModelStatus
{
    private SpeechModelStatus() { }

    public sealed record NotDownloaded : SpeechModelStatus;

    /// <summary>Fraction 0–1.</summary>
    public sealed record Downloading(float Fraction) : SpeechModelStatus;

    public sealed record Ready : SpeechModelStatus;

    public sealed record Failed(string Message) : SpeechModelStatus;
}

/// <summary>An on-device speech model ("Base · 142 MB").</summary>
public sealed record SpeechModelState(string Id, string Name, ulong Bytes, SpeechModelStatus State);

/// <summary>An origin the person allowed a plugin to send audio to.</summary>
public sealed record AudioConsent(string Plugin, string Origin);

/// <summary>The AiLyrics slice.</summary>
/// <param name="Available">Offer Create Synced Lyrics (Pro may still be needed: see Need).</param>
public sealed record AiLyricsSnapshot(
    ulong Version,
    bool Available,
    string? Plugin,
    AiLyricsPlan? Plan,
    AiLyricsNeed Need,
    IReadOnlyList<LyricsJob> Jobs,
    IReadOnlyList<SpeechModelState> Models,
    IReadOnlyList<AudioConsent> Consents);

// ---- the look (ADR-0020) --------------------------------------------------------------------

[JsonConverter(typeof(JsonStringEnumConverter<Typeface>))]
public enum Typeface
{
    System,
    Rounded,
    Serif,
    Mono,
}

[JsonConverter(typeof(JsonStringEnumConverter<Density>))]
public enum Density
{
    Compact,
    Standard,
    Roomy,
}

[JsonConverter(typeof(JsonStringEnumConverter<LyricsWeight>))]
public enum LyricsWeight
{
    Regular,
    Medium,
    Semibold,
    Bold,
    Heavy,
}

/// <summary>The colour of the sung part of lyrics.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(LyricsHighlight.Artwork), "Artwork")]
[JsonDerivedType(typeof(LyricsHighlight.Accent), "Accent")]
[JsonDerivedType(typeof(LyricsHighlight.Custom), "Custom")]
public abstract record LyricsHighlight
{
    private LyricsHighlight() { }

    public sealed record Artwork : LyricsHighlight;

    public sealed record Accent : LyricsHighlight;

    /// <summary>0xRRGGBB.</summary>
    public sealed record Custom(uint Rgb) : LyricsHighlight;
}

[JsonConverter(typeof(JsonStringEnumConverter<Place>))]
public enum Place
{
    Home,
    Albums,
    Artists,
    Songs,
    Playlists,
    Favourites,
    Search,
    Sources,
}

[JsonConverter(typeof(JsonStringEnumConverter<HomeSection>))]
public enum HomeSection
{
    Summary,
    RecentlyAdded,
    RecentlyPlayed,
    MostPlayed,
    Favourites,
}

[JsonConverter(typeof(JsonStringEnumConverter<NowPlayingLayout>))]
public enum NowPlayingLayout
{
    Stage,
    Artwork,
    Lyrics,
}

[JsonConverter(typeof(JsonStringEnumConverter<MiniPlayerStyle>))]
public enum MiniPlayerStyle
{
    Bar,
    Compact,
    Floating,
}

/// <summary>The Look slice: every value resolved (ADR-0020 §2); with Customized false, the
/// Luminous defaults.</summary>
public sealed record LookSnapshot(
    ulong Version,
    float CornerScale,
    float Glass,
    float Aurora,
    Typeface Typeface,
    float TextScale,
    Density Density,
    LyricsWeight LyricsWeight,
    float LyricsGlow,
    LyricsHighlight LyricsHighlight,
    IReadOnlyList<Place> Sidebar,
    IReadOnlyList<HomeSection> Home,
    NowPlayingLayout NowPlaying,
    MiniPlayerStyle MiniPlayer,
    bool Customized,
    string? Plugin);

// ---- Jellyfesh Pro (ADR-0021) ---------------------------------------------------------------

/// <summary>Where Pro comes from.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(ProSource.None), "None")]
[JsonDerivedType(typeof(ProSource.Preview), "Preview")]
[JsonDerivedType(typeof(ProSource.Store), "Store")]
[JsonDerivedType(typeof(ProSource.Licence), "Licence")]
public abstract record ProSource
{
    private ProSource() { }

    public sealed record None : ProSource;

    /// <summary>"Pro features are free during the public beta."</summary>
    public sealed record Preview : ProSource;

    /// <summary>"Jellyfesh Pro · Purchased" (the Rust field is `store`).</summary>
    public sealed record Store([property: JsonPropertyName("store")] global::Jellyfesh.Ffi.Store Value) : ProSource;

    /// <summary>"Jellyfesh Pro · Licence …{IdSuffix}"; issued at midnight UTC, in ms since the epoch.</summary>
    public sealed record Licence(string IdSuffix, long IssuedAtMs) : ProSource;
}

/// <summary>Why a licence key was not accepted (the apps word it).</summary>
[JsonConverter(typeof(JsonStringEnumConverter<LicenceError>))]
public enum LicenceError
{
    Malformed,
    UnknownKey,
    BadSignature,
    WrongProduct,
    Revoked,
}

/// <summary>The Pro slice; Stores are the stores this build can sell Pro in.</summary>
public sealed record ProSnapshot(
    ulong Version,
    bool Active,
    ProSource Source,
    bool Preview,
    LicenceError? LicenceError,
    IReadOnlyList<Store> Stores,
    bool LicenceKeys = true);

// ---- updates (ADR-0021 §5) ------------------------------------------------------------------

/// <summary>A newer release and its page.</summary>
public sealed record UpdateInfo(string Version, string Url);

/// <summary>The Update slice: Supported for direct downloads.</summary>
public sealed record UpdateSnapshot(
    ulong Version,
    bool Supported,
    bool Checking,
    UpdateInfo? Available,
    long? LastCheckedMs,
    bool Auto,
    string? Error);

// ---- Discover, details and comments (ADR-0022) ----------------------------------------------

[JsonConverter(typeof(JsonStringEnumConverter<ShelfStyle>))]
public enum ShelfStyle
{
    Row,
    Grid,
    Chart,
}

/// <summary>One shelf of a Discover page; More is a node to browse for See All.</summary>
public sealed record Shelf(string Id, string Title, ShelfStyle Style, IReadOnlyList<SourceItem> Items, string? More);

/// <summary>A Discover page.</summary>
public sealed record DiscoverPage(ulong Request, string Source, IReadOnlyList<Shelf> Shelves, string? Error);

/// <summary>What a fact is; the apps word the keys.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(FactKey.Version), "Version")]
[JsonDerivedType(typeof(FactKey.Genre), "Genre")]
[JsonDerivedType(typeof(FactKey.Language), "Language")]
[JsonDerivedType(typeof(FactKey.Released), "Released")]
[JsonDerivedType(typeof(FactKey.Label), "Label")]
[JsonDerivedType(typeof(FactKey.Composer), "Composer")]
[JsonDerivedType(typeof(FactKey.Lyricist), "Lyricist")]
[JsonDerivedType(typeof(FactKey.Arranger), "Arranger")]
[JsonDerivedType(typeof(FactKey.Producer), "Producer")]
[JsonDerivedType(typeof(FactKey.Popularity), "Popularity")]
[JsonDerivedType(typeof(FactKey.Quality), "Quality")]
[JsonDerivedType(typeof(FactKey.Isrc), "Isrc")]
[JsonDerivedType(typeof(FactKey.Disc), "Disc")]
[JsonDerivedType(typeof(FactKey.Track), "Track")]
[JsonDerivedType(typeof(FactKey.Other), "Other")]
public abstract record FactKey
{
    private FactKey() { }

    public sealed record Version : FactKey;

    public sealed record Genre : FactKey;

    public sealed record Language : FactKey;

    public sealed record Released : FactKey;

    public sealed record Label : FactKey;

    public sealed record Composer : FactKey;

    public sealed record Lyricist : FactKey;

    public sealed record Arranger : FactKey;

    public sealed record Producer : FactKey;

    public sealed record Popularity : FactKey;

    public sealed record Quality : FactKey;

    public sealed record Isrc : FactKey;

    public sealed record Disc : FactKey;

    public sealed record Track : FactKey;

    /// <summary>The service's own label (the Rust field `label`; Text here, since FactKey.Label is
    /// a sibling record).</summary>
    public sealed record Other([property: JsonPropertyName("label")] string Text) : FactKey;
}

public sealed record Fact(FactKey Key, string Value);

/// <summary>How good what plays is, as the core judges it (the apps word it).</summary>
[JsonConverter(typeof(JsonStringEnumConverter<QualityLabel>))]
public enum QualityLabel
{
    HiResLossless,
    Lossless,
    High,
    Standard,
}

/// <summary>The format of what plays or of a file.</summary>
public sealed record AudioFormat(
    string? Codec,
    uint? SampleRateHz,
    uint? BitDepth,
    uint? Channels,
    uint? BitrateKbps,
    bool? Lossless,
    QualityLabel? Quality);

/// <summary>ReplayGain values: gains in dB, peaks as linear amplitude.</summary>
public sealed record ReplayGainValues(float? TrackGainDb, float? TrackPeak, float? AlbumGainDb, float? AlbumPeak);

/// <summary>Technical information about a file or a stream.</summary>
public sealed record TechnicalInfo(
    AudioFormat Format,
    string? Container,
    ulong? FileSizeBytes,
    ulong? DurationMs,
    ReplayGainValues? Replaygain,
    string? Path);

/// <summary>An item's details (Event.Details).</summary>
/// <param name="Image">A large cover or an artist photo: a cached file, or a URL for SourceArtwork.</param>
public sealed record DetailsPage(
    ulong Request,
    ItemRef Item,
    string Title,
    string? Subtitle,
    string? Description,
    ArtworkRef? Image,
    IReadOnlyList<Fact> Facts,
    TechnicalInfo? Technical,
    ulong? CommentCount,
    string? Attribution,
    string? Error);

/// <summary>A comment on a source item; TimeMs in ms since the epoch.</summary>
public sealed record CommentEntry(
    string Id,
    string Author,
    ArtworkRef? Avatar,
    string Text,
    ulong? Likes,
    long? TimeMs,
    ulong? Replies);

/// <summary>A page of comments; ask for Next with Command.Comments.</summary>
public sealed record CommentsPage(
    ulong Request,
    ItemRef Item,
    IReadOnlyList<CommentEntry> Hot,
    IReadOnlyList<CommentEntry> Comments,
    ulong? Total,
    string? Next,
    string? Attribution,
    string? Error);

/// <summary>A source picture fetched into the artwork cache; Path null when it cannot be had.</summary>
public sealed record ArtworkReady(string Url, uint SizePx, string? Path);

/// <summary>What the lyrics bar shows at a time (JellyfeshMethods.LyricsBar): the line, the next
/// line, each word's progress (0–1) and, for lines without word timing, the line's progress.</summary>
public sealed record LyricsBar(uint? Line, uint? Next, IReadOnlyList<float> Words, float LineProgress);

// ---- v0.0.8: source libraries and downloads (ADR-0024, ADR-0025) ------------------------------

/// <summary>One of a source's lists.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<SourceLibraryPart>))]
public enum SourceLibraryPart
{
    LikedSongs,
    LikedAlbums,
    LikedArtists,
    Playlists,
}

/// <summary>An entry of a source's list.</summary>
/// <param name="Owned">Playlists: the person's own (or collaborative), not followed.</param>
/// <param name="SongsListed">Playlists: the source lists its songs here; when false the page says
/// "{Service} lists this playlist's songs only in {Service}." and Play starts it by context.</param>
public sealed record SourceLibraryEntry(SourceItem Item, bool Owned, bool SongsListed);

/// <summary>One list of a source: the first page (at most 100), kept with its age.</summary>
/// <param name="Next">SourceLibraryMore's cursor.</param>
/// <param name="RefreshedAtMs">When it was last loaded ("Updated 5 minutes ago").</param>
public sealed record SourceLibraryList(
    SourceLibraryPart Part,
    IReadOnlyList<SourceLibraryEntry> Entries,
    uint? Total,
    string? Next,
    long? RefreshedAtMs,
    bool Loading,
    string? Error);

/// <summary>A source's likes and playlists, in the order Liked Songs, Liked Albums, Liked
/// Artists, Playlists; lists it does not keep are left out.</summary>
public sealed record SourceLibrary(string Plugin, string Name, string? Attribution, IReadOnlyList<SourceLibraryList> Lists);

/// <summary>The SourceLibrary slice: every enabled, signed-in source with the library
/// capability.</summary>
public sealed record SourceLibrarySnapshot(ulong Version, IReadOnlyList<SourceLibrary> Sources);

/// <summary>More of a source's list (Command.SourceLibraryMore).</summary>
public sealed record SourceLibraryPage(
    ulong Request,
    string Plugin,
    SourceLibraryPart Part,
    IReadOnlyList<SourceLibraryEntry> Entries,
    string? Next,
    string? Error);

/// <summary>Why downloads wait.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<DownloadsPaused>))]
public enum DownloadsPaused
{
    /// <summary>No network.</summary>
    Offline,
    /// <summary>Metered with Wi-Fi Only on ("Waiting for Wi-Fi").</summary>
    Metered,
}

/// <summary>Why a download failed; the apps word it.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(DownloadError.NotAllowed), "NotAllowed")]
[JsonDerivedType(typeof(DownloadError.NotOffered), "NotOffered")]
[JsonDerivedType(typeof(DownloadError.Network), "Network")]
[JsonDerivedType(typeof(DownloadError.Storage), "Storage")]
[JsonDerivedType(typeof(DownloadError.Other), "Other")]
public abstract record DownloadError
{
    private DownloadError() { }

    /// <summary>"Your {Service} server doesn't allow downloads for this account."</summary>
    public sealed record NotAllowed(string Service) : DownloadError;

    /// <summary>The source offers no downloads.</summary>
    public sealed record NotOffered(string Service) : DownloadError;

    /// <summary>The connection failed after the retries.</summary>
    public sealed record Network(string Message) : DownloadError;

    /// <summary>The file could not be written.</summary>
    public sealed record Storage(string Message) : DownloadError;

    public sealed record Other(string Message) : DownloadError;
}

/// <summary>Where a download is.</summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(DownloadState.Queued), "Queued")]
[JsonDerivedType(typeof(DownloadState.Downloading), "Downloading")]
[JsonDerivedType(typeof(DownloadState.Done), "Done")]
[JsonDerivedType(typeof(DownloadState.Failed), "Failed")]
public abstract record DownloadState
{
    private DownloadState() { }

    /// <summary>Waiting for its turn, or for the network (DownloadsSnapshot.Paused).</summary>
    public sealed record Queued : DownloadState;

    public sealed record Downloading : DownloadState;

    public sealed record Done : DownloadState;

    /// <summary>Try Again sends RetryDownload.</summary>
    public sealed record Failed(DownloadError Error) : DownloadState;
}

/// <summary>One download.</summary>
/// <param name="Path">The file, once finished.</param>
/// <param name="LibraryId">Its library track, once finished.</param>
/// <param name="Original">The server's original rather than a conversion.</param>
public sealed record DownloadItem(
    ulong Id,
    SourceTrack Track,
    DownloadState State,
    ulong BytesDone,
    ulong? BytesTotal,
    string? Path,
    long? LibraryId,
    bool Original);

/// <summary>The Downloads slice.</summary>
/// <param name="Folder">Where downloads go: the person's folder, or the default one.</param>
/// <param name="Items">In progress (queue order), then finished by album.</param>
/// <param name="BytesUsed">Space the finished downloads take.</param>
public sealed record DownloadsSnapshot(
    ulong Version,
    string Folder,
    IReadOnlyList<DownloadItem> Items,
    ulong BytesUsed,
    DownloadsPaused? Paused);
