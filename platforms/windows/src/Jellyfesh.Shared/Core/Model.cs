// The core's data as the Windows shell sees it. These mirror core/jf-core/src/types.rs
// (and the jf-engine, jf-library, jf-session and jf-lyrics types it re-exports), but are
// independent of the generated bindings: only Jellyfesh.Interop maps one onto the other.

namespace Jellyfesh.Core;

/// <summary>What the queue plays: a library track, a file, or a track from an online source.</summary>
public abstract record TrackRef
{
    private TrackRef() { }

    public sealed record Library(long Id) : TrackRef;

    public sealed record File(string Path) : TrackRef;

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

/// <summary>One queue entry; <see cref="Uid"/> is stable while the entry stays queued.</summary>
public sealed record QueueItem(ulong Uid, TrackRef Track);

public enum RepeatMode
{
    Off,
    All,
    One,
}

public enum PlaybackStatus
{
    Stopped,
    Playing,
    Paused,
    Ended,
}

/// <summary>
/// The time base for progress and lyrics (architecture §5):
/// position(now) = PositionUs + (now − AnchorMonoNs) × Rate / 1000.
/// </summary>
public readonly record struct PlaybackAnchor(
    ulong TrackGeneration,
    long PositionUs,
    ulong AnchorMonoNs,
    float Rate,
    long? DurationUs);

public enum EqBandKind
{
    Peaking,
    LowShelf,
    HighShelf,
}

public sealed record EqBand(EqBandKind Kind, float FreqHz, float GainDb, float Q, bool Enabled);

/// <summary>
/// The equalizer: on or off, a preamp and the core's bands. The band layout and the presets come
/// from the core (Settings.Eq and <see cref="ICoreClient.EqPresets"/>), never from a copy here.
/// </summary>
public sealed record EqSettings(bool Enabled, float PreampDb, IReadOnlyList<EqBand> Bands)
{
    /// <summary>Off and without bands: the placeholder until the core's settings arrive.</summary>
    public static EqSettings None { get; } = new(false, 0f, []);

    public bool ContentEquals(EqSettings? other) =>
        other is not null
        && Enabled == other.Enabled
        && PreampDb.Equals(other.PreampDb)
        && Bands.SequenceEqual(other.Bands);

    /// <summary>
    /// Whether the bands have the gains of <paramref name="other"/>'s, band by band (to 0.05 dB),
    /// whatever the preamp and the switch: how the Preset menu tells a preset from Custom.
    /// </summary>
    public bool SameGains(EqSettings other) =>
        Bands.Count == other.Bands.Count
        && Bands.Zip(other.Bands).All(p => Math.Abs(p.First.GainDb - p.Second.GainDb) < 0.05f);
}

/// <summary>
/// An equalizer preset from the core (<see cref="ICoreClient.EqPresets"/>), in the order every app
/// lists it. <see cref="Id"/> is flat, bass_boost, vocal or treble; the app shows it translated.
/// </summary>
public sealed record EqPreset(string Id, EqSettings Settings);

public enum ReplayGainMode
{
    Off,
    Track,
    Album,
}

/// <summary>Facts about an opened track; <see cref="Bitrate"/> in bits per second.</summary>
/// <param name="BitsPerSample">PCM, FLAC and ALAC.</param>
public sealed record TrackFormat(string Codec, uint SampleRate, ushort Channels, long? DurationUs, bool Gapless, uint? Bitrate, uint? BitsPerSample = null);

/// <summary>
/// Artwork: a cached file to load, or a source picture's URL, which only the core fetches
/// (docs/core-api.md, "Covers"; ThumbnailService.GetSourceAsync). A URL is never loaded directly:
/// it may be a web address, or a jellyfesh-artwork:// reference the core resolves through the plugin.
/// </summary>
public abstract record ArtworkRef
{
    private ArtworkRef() { }

    public sealed record File(string Path) : ArtworkRef;

    public sealed record Url(string Address) : ArtworkRef;
}

/// <summary>Local paths as file URIs an image control can load.</summary>
public static class FileUris
{
    /// <summary>
    /// The file URI of <paramref name="path"/>, or null when it cannot be one. A rooted path
    /// without a drive (<c>/cache/a.jpg</c>) is completed with the current drive on Windows,
    /// where <see cref="Uri(string)"/> would throw.
    /// </summary>
    public static string? From(string path)
    {
        if (string.IsNullOrEmpty(path))
        {
            return null;
        }

        if (Uri.TryCreate(path, UriKind.Absolute, out var uri) && uri.IsFile)
        {
            return uri.AbsoluteUri;
        }

        try
        {
            return Uri.TryCreate(System.IO.Path.GetFullPath(path), UriKind.Absolute, out var full) && full.IsFile
                ? full.AbsoluteUri
                : null;
        }
        catch (Exception e) when (e is ArgumentException or NotSupportedException or System.IO.PathTooLongException)
        {
            return null;
        }
    }
}

/// <summary>One audio file in the library catalogue.</summary>
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
    string? Codec,
    string? ArtworkHash);

public sealed record Album(long Id, string Title, string? AlbumArtist, int? Year, uint TrackCount, string? ArtworkHash);

public sealed record Artist(long Id, string Name, uint TrackCount);

public enum AlbumSort
{
    Title,
    Artist,
    Year,
    Added,
}

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

public sealed record TrackQuery(
    uint Offset = 0,
    uint Limit = 100,
    TrackSort Sort = TrackSort.Artist,
    bool Descending = false,
    long? AlbumId = null,
    long? ArtistId = null);

public sealed record SearchResults(IReadOnlyList<Track> Tracks, IReadOnlyList<Album> Albums, IReadOnlyList<Artist> Artists)
{
    public static SearchResults Empty { get; } = new([], [], []);

    public bool IsEmpty => Tracks.Count == 0 && Albums.Count == 0 && Artists.Count == 0;
}

public sealed record LibraryStats(
    ulong Roots,
    ulong Tracks,
    ulong Albums,
    ulong Artists,
    ulong Artwork,
    ulong TotalDurationMs,
    ulong TotalBytes);

/// <summary>A row in a track list (playlist, favourites, history).</summary>
public sealed record TrackRow(
    TrackRef Track,
    string Title,
    string? Artist,
    string? Album,
    ulong? DurationMs,
    ArtworkRef? Artwork,
    string? ArtworkHash,
    bool Favorite,
    bool Missing);

/// <summary>A queue entry that started playing.</summary>
public sealed record HistoryEntry(TrackRef Track, long PlayedAtMs);

public sealed record PlaylistSummary(
    long Id,
    string Name,
    uint EntryCount,
    ulong DurationMs,
    long UpdatedAtMs,
    string? ArtworkHash);

public sealed record PlaylistEntry(long EntryId, uint Position, TrackRow Row);

/// <summary>Reported once per audio file a scan visits.</summary>
public sealed record ScanProgress(ulong FilesSeen, ulong FilesChanged, string Current);

public sealed record ScanReport(ulong Added, ulong Updated, ulong Removed, ulong Unchanged, IReadOnlyList<ScanError> Errors);

public sealed record ScanError(string Path, string Message);

public abstract record ScanStatus
{
    private ScanStatus() { }

    public sealed record Progress(ScanProgress Value) : ScanStatus;

    public sealed record Finished(ScanReport Report) : ScanStatus;

    public sealed record Failed(string Message) : ScanStatus;
}

/// <summary>An audio output device; <see cref="Id"/> is stable and is what SetOutputDevice takes.</summary>
public sealed record DeviceInfo(string Id, string Name, bool IsDefault, IReadOnlyList<uint> SampleRates);

/// <summary>How a source signs in (ADR-0004).</summary>
public enum AuthMethod
{
    Qr,
    Oauth,
    CookieImport,
    PasswordExchange,
    ApiKey,
}

/// <summary>The badge a source shows (ADR-0004, ADR-0012).</summary>
public enum PluginBadge
{
    Official,
    Community,
}

/// <summary>Where Settings lists a plugin: Sources, or Extensions (AI Lyrics, Customize).</summary>
public enum PluginKind
{
    Source,

    /// <summary>Makes lyrics or changes the look, and offers no music.</summary>
    Extension,
}

/// <summary>One installed source or extension.</summary>
/// <param name="RemotePlayback">The service plays on the person's own devices and Jellyfesh is its
/// remote control (ADR-0018): its songs start a remote session and never join the queue, so Play
/// Next, Play Last and Add to Playlist are not offered for them.</param>
/// <param name="Attribution">Set when the service's terms ask for credit: its views then say
/// "Content from {Service}" (docs/ui.md, "Links and remote playback").</param>
/// <param name="Kind">Sources, or Settings › Extensions.</param>
/// <param name="RequiresPro">Its features need Jellyfesh Pro (it stays installed and its settings work without).</param>
/// <param name="Permissions">The host imports the manifest declares (audio, speech).</param>
/// <param name="Discover">It makes its own Discover shelves.</param>
/// <param name="Details">It answers Details for its items.</param>
/// <param name="Comments">It has comments: Now Playing shows the Comments pane for its songs.</param>
/// <param name="Description">One line from the manifest.</param>
public sealed record PluginInfo(
    string Id,
    string Name,
    string Version,
    string Author,
    PluginBadge Badge,
    bool VendorPlayback,
    IReadOnlyList<string> Capabilities,
    IReadOnlyList<AuthMethod> AuthMethods,
    bool Enabled,
    string? Account,
    bool NeedsDisclosure,
    string SettingsSchemaJson,
    string SettingsJson,
    string? Icon,
    bool RemotePlayback = false,
    string? Attribution = null,
    PluginKind Kind = PluginKind.Source,
    bool RequiresPro = false,
    IReadOnlyList<string>? Permissions = null,
    bool Discover = false,
    bool Details = false,
    bool Comments = false,
    string? Description = null);

/// <summary>
/// A music service offered in Settings → Sources → Add a Source, listed while its official plugin
/// is installed; <see cref="CoreCommand.ConnectSource"/> connects to it.
/// </summary>
/// <param name="Id">Stable name the app keys the description by: jellyfin, navidrome, subsonic.</param>
/// <param name="Plugin">The plugin that connects to it; the sign-in steps arrive as its Auth events.</param>
/// <param name="NeedsAddress">Whether the person types the server's address (a self-hosted server).</param>
/// <param name="Address">The address set now, to fill the field with.</param>
/// <param name="AddressExample">An address to show as an example in that field.</param>
/// <param name="Account">The account the plugin is signed in with.</param>
public sealed record SourcePreset(
    string Id,
    string Name,
    string Plugin,
    IReadOnlyList<AuthMethod> AuthMethods,
    bool NeedsAddress,
    string? Address,
    string? AddressExample,
    string? Account);

/// <summary>How a sign-in field is entered: the keyboard and the field it gets.</summary>
public enum AuthFieldKind
{
    Text,
    Email,
    Url,
    Number,

    /// <summary>A Cookie header pasted from a browser: several lines.</summary>
    CookieHeader,
}

/// <summary>A field to ask for during sign-in, as the plugin describes it.</summary>
/// <param name="Required">Sign In waits until it has something in it.</param>
/// <param name="Help">A sentence shown under the field.</param>
public sealed record AuthField(
    string Name,
    string Label,
    bool Secret,
    string? Placeholder,
    AuthFieldKind Kind = AuthFieldKind.Text,
    bool Required = true,
    string? Help = null);

/// <summary>One step of a sign-in flow.</summary>
public abstract record AuthStep
{
    private AuthStep() { }

    /// <summary>A QR code of <paramref name="Data"/> to scan; <paramref name="Hint"/> is the plugin's sentence under it.</summary>
    public sealed record ShowQr(string Data, long? ExpiresAtMs, string? Hint = null) : AuthStep;

    /// <summary>A code shown large, to enter on another device (Jellyfin's Quick Connect); <paramref name="Hint"/> says where.</summary>
    public sealed record ShowCode(string Code, string Hint, long? ExpiresAtMs) : AuthStep;

    public sealed record OpenUrl(string Url) : AuthStep;

    public sealed record AskFields(IReadOnlyList<AuthField> Fields) : AuthStep;

    public sealed record Waiting(string Message) : AuthStep;

    public sealed record Done(string Account) : AuthStep;

    public sealed record Failed(string Message) : AuthStep;
}

public sealed record AuthUpdate(string Plugin, ulong Flow, AuthStep Step);

public enum ItemKind
{
    Track,
    Album,
    Artist,
    Playlist,
    Folder,
}

/// <summary>A search or browse result from the library or a source.</summary>
/// <param name="Link">An https link that opens the item in its service (Open in {Service}), when
/// its source declares one; the main action for an item that cannot play here.</param>
/// <param name="ArtistId">The source's id of a track's or album's artist: Go to Artist.</param>
/// <param name="AlbumId">The source's id of a track's album: Go to Album.</param>
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
    string? AlbumId = null);

/// <summary>Results from one source for one search.</summary>
public sealed record SearchUpdate(
    ulong Request,
    string Query,
    string Source,
    IReadOnlyList<SourceItem> Items,
    string? Error,
    bool Done);

/// <summary>One page of a source's catalogue.</summary>
public sealed record BrowseUpdate(
    ulong Request,
    string Plugin,
    string? Node,
    IReadOnlyList<SourceItem> Items,
    uint? NextPage,
    string? Error);

/// <summary>A group of tracks that plays as a unit.</summary>
public abstract record Collection
{
    private Collection() { }

    public sealed record Album(long Id) : Collection;

    public sealed record Artist(long Id) : Collection;

    public sealed record Playlist(long Id) : Collection;

    public sealed record Favorites : Collection;

    /// <summary>Every song in the library, in the Songs page's order: Play and Shuffle with no paging.</summary>
    public sealed record Songs(TrackSort Sort, bool Descending) : Collection;

    public sealed record Tracks(IReadOnlyList<TrackRef> Items) : Collection;

    public sealed record Source(string Plugin, string Id) : Collection;
}

/// <summary>Where a search looks.</summary>
public abstract record SearchScope
{
    private SearchScope() { }

    public sealed record All : SearchScope;

    public sealed record Library : SearchScope;

    public sealed record Plugin(string Id) : SearchScope;
}

/// <summary>What kind of device a remote source plays on; Devices shows its glyph.</summary>
public enum RemoteDeviceKind
{
    Computer,
    Smartphone,
    Speaker,
    Tv,
    Car,
    Other,
}

/// <summary>A device a remote source can play on (ADR-0018).</summary>
/// <param name="Id">What <see cref="RemoteAction.Transfer"/> takes.</param>
/// <param name="Active">The device playing (or last playing) for the account.</param>
/// <param name="VolumePercent">Its volume, 0–100, when it lets Jellyfesh change it.</param>
public sealed record RemoteDevice(string Id, string Name, RemoteDeviceKind Kind, bool Active, uint? VolumePercent);
