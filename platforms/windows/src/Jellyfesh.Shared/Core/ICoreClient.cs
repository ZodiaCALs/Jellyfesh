namespace Jellyfesh.Core;

/// <summary>
/// The only way the Windows shell reaches the Rust core (ADR-0001). Jellyfesh.Interop implements
/// it over the Jellyfesh.Ffi binding; tests use a fake. Queries run synchronously on the caller's
/// thread, so view models call them off the UI thread. They throw <see cref="CoreQueryException"/>.
/// </summary>
public interface ICoreClient : IDisposable
{
    /// <summary>Queues a command and returns its id (0 after shutdown).</summary>
    ulong Send(CoreCommand command);

    /// <summary>The current state of every slice.</summary>
    CoreSnapshot Snapshot();

    /// <summary>
    /// Receives every event, in order, on the core's event thread. Set it before reading the
    /// snapshot and before sending commands whose events you need. Null stops delivery.
    /// </summary>
    void SetListener(Action<CoreEvent>? listener);

    IReadOnlyList<Album> Albums(uint offset, uint limit);

    IReadOnlyList<Album> AlbumsSorted(AlbumSort sort, bool descending, uint offset, uint limit);

    IReadOnlyList<Artist> Artists(uint offset, uint limit);

    IReadOnlyList<Track> Tracks(TrackQuery query);

    /// <summary>One library track, for rows that hold only a <see cref="TrackRef.Library"/> id.</summary>
    Track? Track(long id);

    IReadOnlyList<Track> AlbumTracks(long albumId);

    IReadOnlyList<Album> ArtistAlbums(long artistId);

    /// <summary>Library-only search, fast enough for search-as-you-type.</summary>
    SearchResults SearchLibrary(string text, uint limit);

    /// <summary>Library tracks to play for a search (an exact album or artist plays it whole).</summary>
    IReadOnlyList<TrackRef> TracksForQuery(string text, uint limit);

    IReadOnlyList<Track> RecentlyAdded(uint limit);

    IReadOnlyList<TrackRow> RecentlyPlayed(uint limit);

    IReadOnlyList<TrackRow> MostPlayed(uint limit);

    IReadOnlyList<PlaylistSummary> Playlists();

    PlaylistSummary? Playlist(long id);

    IReadOnlyList<PlaylistEntry> PlaylistEntries(long id, uint offset, uint limit);

    /// <summary>Writes the playlist to <paramref name="path"/> as M3U8.</summary>
    void ExportPlaylist(long id, string path);

    IReadOnlyList<TrackRow> Favorites(uint offset, uint limit);

    bool IsFavorite(TrackRef track);

    /// <summary>A square JPEG of the artwork with this hash at about <paramref name="sizePx"/>, made on first use.</summary>
    string? ArtworkThumbnail(string hash, uint sizePx);

    IReadOnlyList<QueueItem> QueueWindow(uint offset, uint limit);

    IReadOnlyList<HistoryEntry> History();

    IReadOnlyList<string> Roots();

    LibraryStats Stats();

    /// <summary>Audio output devices, for a device picker.</summary>
    IReadOnlyList<DeviceInfo> OutputDevices();

    /// <summary>
    /// The equalizer presets (the core's eq_presets()), in the order every app lists them. Applying
    /// one sends its settings as Settings.Eq; no app keeps a copy of the gains.
    /// </summary>
    IReadOnlyList<EqPreset> EqPresets();

    /// <summary>The clock anchors are stamped with, in nanoseconds. Not for per-frame use: see CoreClock.</summary>
    ulong MonotonicNowNs();

    /// <summary>Saves the session and stops the core.</summary>
    void Shutdown();
}

/// <summary>Why a query failed (jf-ffi's CoreFfiError kinds).</summary>
public enum CoreQueryError
{
    /// <summary>A library, session, audio or file error; the message says which.</summary>
    Core,

    /// <summary>The feature is still being integrated in this release.</summary>
    NotAvailable,

    /// <summary>The core was shut down.</summary>
    ShutDown,

    /// <summary>An argument could not be used.</summary>
    InvalidArgument,
}

/// <summary>A query the core could not answer.</summary>
public sealed class CoreQueryException : Exception
{
    public CoreQueryException()
    {
    }

    public CoreQueryException(string message)
        : base(message)
    {
    }

    public CoreQueryException(string message, Exception inner)
        : base(message, inner)
    {
    }

    public CoreQueryException(CoreQueryError kind, string message, Exception? inner = null)
        : base(message, inner)
    {
        Kind = kind;
    }

    public CoreQueryError Kind { get; }

    /// <summary>The feature is not in this build yet: show "coming soon", not an error.</summary>
    public bool IsNotYetAvailable => Kind == CoreQueryError.NotAvailable;
}
