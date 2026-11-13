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

public sealed record EqSettings(bool Enabled, float PreampDb, IReadOnlyList<EqBand> Bands)
{
    /// <summary>The ISO octave centres the core's presets use.</summary>
    public static readonly float[] PresetFrequencies =
        [31f, 62f, 125f, 250f, 500f, 1_000f, 2_000f, 4_000f, 8_000f, 16_000f];

    /// <summary>One octave wide, as in jf-engine's presets.</summary>
    public const float PresetQ = 1.41f;

    /// <summary>Ten one-octave peaking bands at 0 dB, switched off (jf-engine's default).</summary>
    public static EqSettings Flat { get; } = Graphic(new float[10]) with { Enabled = false };

    /// <summary>A ten-band graphic EQ at the preset frequencies.</summary>
    public static EqSettings Graphic(IReadOnlyList<float> gainsDb) =>
        new(true, 0f, PresetFrequencies
            .Select((f, i) => new EqBand(EqBandKind.Peaking, f, i < gainsDb.Count ? gainsDb[i] : 0f, PresetQ, true))
            .ToArray());

    public bool ContentEquals(EqSettings? other) =>
        other is not null
        && Enabled == other.Enabled
        && PreampDb.Equals(other.PreampDb)
        && Bands.SequenceEqual(other.Bands);
}

public enum ReplayGainMode
{
    Off,
    Track,
    Album,
}

/// <summary>Facts about an opened track.</summary>
public sealed record TrackFormat(string Codec, uint SampleRate, ushort Channels, long? DurationUs, bool Gapless, uint? Bitrate);

/// <summary>Artwork a UI can load: a cached file, or a URL for online items.</summary>
public abstract record ArtworkRef
{
    private ArtworkRef() { }

    public sealed record File(string Path) : ArtworkRef;

    public sealed record Url(string Address) : ArtworkRef;

    /// <summary>A URI an image control can load.</summary>
    public string ToUriString() => this switch
    {
        File f => new Uri(f.Path).AbsoluteUri,
        Url u => u.Address,
        _ => string.Empty,
    };
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

/// <summary>One installed source.</summary>
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
    string? Icon);

/// <summary>A field to ask for during sign-in.</summary>
public sealed record AuthField(string Name, string Label, bool Secret, string? Placeholder);

/// <summary>One step of a sign-in flow.</summary>
public abstract record AuthStep
{
    private AuthStep() { }

    public sealed record ShowQr(string Data, long? ExpiresAtMs) : AuthStep;

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
    IReadOnlyList<string> AlsoIn);

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
