namespace Jellyfesh.Core;

// v0.0.8 (ADR-0024 to ADR-0026): streaming quality, the mini player's settings, the sources' likes
// and playlists, search by source, and downloads. Mirrors of the core's types; the adapter
// (Jellyfesh.Interop/FfiMapping.cs) is the only code that sees the binding's.

/// <summary>The streaming level asked of sources (ADR-0026 §1): Auto is Lossless, or High on a metered connection.</summary>
public enum StreamingQuality
{
    Auto,
    Lossless,
    High,
    Normal,
    Low,
}

/// <summary>The quality downloads are asked for (ADR-0025 §2).</summary>
public enum DownloadQuality
{
    /// <summary>The file exactly as the server keeps it.</summary>
    Original,
    High,
    Normal,
    Low,
}

/// <summary>Settings › Downloads (ADR-0025 §2). A null <paramref name="Folder"/> is the default (the Downloads slice's folder).</summary>
public sealed record DownloadSettings(
    string? Folder,
    DownloadQuality Quality,
    string NameTemplate,
    bool AddMissingCoverAndLyrics,
    bool LyricsInTags,
    bool SaveLrcFiles,
    bool WifiOnly,
    uint AtOnce)
{
    public const uint MinAtOnce = 1;
    public const uint MaxAtOnce = 4;

    /// <summary>The core's defaults, which settings saved before v0.0.8 read as.</summary>
    public static DownloadSettings Default { get; } = new(
        null, DownloadQuality.Original, "{album_artist}/{album}/{track} {title}", false, true, false, true, 2);
}

/// <summary>Settings › Appearance › Mini Player (ADR-0026 §4).</summary>
public sealed record MiniPlayerSettings(bool KeepOnTop, bool OpenWhenWindowClosed, bool ShowLyricsLine, bool AndroidPip)
{
    /// <summary>The core's defaults, which settings saved before v0.0.8 read as.</summary>
    public static MiniPlayerSettings Default { get; } = new(KeepOnTop: true, OpenWhenWindowClosed: false, ShowLyricsLine: true, AndroidPip: false);
}

/// <summary>Why the quality menu offers no levels; the app words it.</summary>
public abstract record QualityNote
{
    private QualityNote() { }

    /// <summary>"Files play as they are."</summary>
    public sealed record FilesPlayAsTheyAre : QualityNote;

    /// <summary>"Quality is set in {Service}."</summary>
    public sealed record SetInService(string Service) : QualityNote;
}

// ---- the sources' likes and playlists (ADR-0024) ---------------------------------------------

/// <summary>One of a source's lists.</summary>
public enum SourceLibraryPart
{
    LikedSongs,
    LikedAlbums,
    LikedArtists,
    Playlists,
}

/// <summary>An entry of a source's list.</summary>
/// <param name="Owned">Playlists: the person's own (or collaborative), not followed.</param>
/// <param name="SongsListed">Playlists: false when the service lists the songs only in its own app.</param>
public sealed record SourceLibraryEntry(SourceItem Item, bool Owned, bool SongsListed);

/// <summary>One list of a source: its first page, kept with its age ("Updated 5 minutes ago").</summary>
public sealed record SourceLibraryList(
    SourceLibraryPart Part,
    IReadOnlyList<SourceLibraryEntry> Entries,
    uint? Total,
    string? Next,
    long? RefreshedAtMs,
    bool Loading,
    string? Error);

/// <summary>A source's likes and playlists, in the order Liked Songs, Liked Albums, Liked Artists, Playlists.</summary>
public sealed record SourceLibrary(string Plugin, string Name, string? Attribution, IReadOnlyList<SourceLibraryList> Lists);

/// <summary>The SourceLibrary slice: every enabled, signed-in source with the library capability.</summary>
public sealed record SourceLibraryState(ulong Version, IReadOnlyList<SourceLibrary> Sources)
{
    public static SourceLibraryState Initial { get; } = new(0, []);
}

/// <summary>More of a source's list, answering <see cref="CoreCommand.SourceLibraryMore"/> (the core's <c>SourceLibraryPage</c>).</summary>
public sealed record SourceLibraryMorePage(
    ulong Request,
    string Plugin,
    SourceLibraryPart Part,
    IReadOnlyList<SourceLibraryEntry> Entries,
    string? Next,
    string? Error);

// ---- search by source (ADR-0024 §4) ----------------------------------------------------------

/// <summary>One kind of a source's search results, for its tab; <paramref name="Next"/> is Show More's cursor.</summary>
public sealed record SearchGroup(ItemKind Kind, IReadOnlyList<SourceItem> Items, string? Next, uint? Total);

/// <summary>
/// More of one kind of a source's results, answering <see cref="CoreCommand.SearchMore"/> (the core's
/// <c>SearchPage</c>, renamed here because the app has a SearchPage view).
/// </summary>
public sealed record SearchMorePage(
    ulong Request,
    string Query,
    string Source,
    ItemKind Kind,
    IReadOnlyList<SourceItem> Items,
    string? Next,
    string? Error);

// ---- downloads (ADR-0025) --------------------------------------------------------------------

/// <summary>Why downloads wait.</summary>
public enum DownloadsPaused
{
    /// <summary>No network.</summary>
    Offline,

    /// <summary>Metered with Wi-Fi Only on.</summary>
    Metered,
}

/// <summary>Why a download failed; the app words it.</summary>
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
public abstract record DownloadState
{
    private DownloadState() { }

    /// <summary>Waiting for its turn, or for the network (<see cref="DownloadsState.Paused"/>).</summary>
    public sealed record Queued : DownloadState;

    public sealed record Downloading : DownloadState;

    public sealed record Done : DownloadState;

    /// <summary>Try Again sends <see cref="CoreCommand.RetryDownload"/>.</summary>
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

/// <summary>The Downloads slice: in progress (queue order), then finished by album.</summary>
public sealed record DownloadsState(ulong Version, string Folder, IReadOnlyList<DownloadItem> Items, ulong BytesUsed, DownloadsPaused? Paused)
{
    public static DownloadsState Initial { get; } = new(0, string.Empty, [], 0, null);
}

/// <summary>What <see cref="CoreCommand.Download"/> downloads.</summary>
public abstract record DownloadTarget
{
    private DownloadTarget() { }

    /// <summary>One song (a source's track).</summary>
    public sealed record Track(TrackRef Value) : DownloadTarget;

    /// <summary>Every song of an album or playlist a source listed.</summary>
    public sealed record Collection(string Plugin, string Id, ItemKind Kind) : DownloadTarget;
}
