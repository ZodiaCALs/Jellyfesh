using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Jellyfesh.Core;
using F = Jellyfesh.Ffi;

namespace Jellyfesh.Interop;

/// <summary>
/// <see cref="ICoreClient"/> over Jellyfesh.Ffi's <c>JellyfeshCore</c> (docs/ffi.md). Events are
/// mapped on the core's event thread and handed to the listener there; queries map
/// <c>CoreFfiException</c> to <see cref="CoreQueryException"/> with the same kind.
/// </summary>
public sealed class FfiCoreClient : ICoreClient
{
    private readonly F.JellyfeshCore _core;
    private bool _disposed;

    private FfiCoreClient(F.JellyfeshCore core) => _core = core;

    /// <summary>
    /// Starts the core in the platform's default folders (or the given ones) with the default
    /// output device. <paramref name="bundledPlugins"/> are the official plugin packages shipped
    /// inside the app (Config.bundled_plugins): the core installs them on first start and updates
    /// them with the app. Throws <see cref="CoreQueryException"/> when it cannot start.
    /// </summary>
    public static FfiCoreClient Start(string? dataDir = null, string? cacheDir = null, IReadOnlyList<string>? bundledPlugins = null)
    {
        try
        {
            if (dataDir is null || cacheDir is null)
            {
                var (data, cache) = DefaultFolders();
                dataDir ??= data;
                cacheDir ??= cache;
            }

            return new FfiCoreClient(new F.JellyfeshCore(Config(dataDir, cacheDir, bundledPlugins ?? [])));
        }
        catch (F.CoreFfiException e)
        {
            throw Map(e);
        }
    }

    /// <summary>
    /// The library's and the cache's folders: the core's defaults (on Windows %APPDATA%\Jellyfesh and
    /// %LOCALAPPDATA%\Jellyfesh\cache; JF_DATA_DIR and JF_CACHE_DIR override them), or the same
    /// places through the known-folder API when the environment does not name them. Throws
    /// <see cref="CoreQueryException"/> when there is neither.
    /// </summary>
    public static (string Data, string Cache) DefaultFolders()
    {
        try
        {
            var data = F.JellyfeshMethods.DefaultDataDir()
                ?? KnownFolder(Environment.SpecialFolder.ApplicationData, "Jellyfesh")
                ?? throw new CoreQueryException(CoreQueryError.Core, "no data folder");
            var cache = F.JellyfeshMethods.DefaultCacheDir()
                ?? KnownFolder(Environment.SpecialFolder.LocalApplicationData, "Jellyfesh", "cache")
                ?? throw new CoreQueryException(CoreQueryError.Core, "no cache folder");
            return (data, cache);
        }
        catch (F.CoreFfiException e)
        {
            throw Map(e);
        }
    }

    private static string? KnownFolder(Environment.SpecialFolder folder, params string[] parts)
    {
        var root = Environment.GetFolderPath(folder);
        return string.IsNullOrEmpty(root) ? null : Path.Combine([root, .. parts]);
    }

    /// <summary>Whether this build passes bundled plugins to the core (the binding has the field).</summary>
    public static bool PassesBundledPlugins =>
#if JF_FFI_BUNDLED_PLUGINS
        true;
#else
        false;
#endif

    /// <summary>
    /// How this build reached the person (ADR-0021 §4), from the MSBuild constants that
    /// JfDistribution (JF_DISTRIBUTION) sets: the release page's downloads check for updates, the
    /// Microsoft Store build sells Pro through the Store and installs no plugins from files.
    /// </summary>
    public static F.Distribution Distribution =>
#if JF_DISTRIBUTION_DIRECT
        F.Distribution.Direct;
#elif JF_DISTRIBUTION_MICROSOFT_STORE
        F.Distribution.MicrosoftStore;
#else
        F.Distribution.Source;
#endif

    internal static F.CoreConfig Config(string dataDir, string cacheDir, IReadOnlyList<string> bundledPlugins)
    {
        var config = new F.CoreConfig(dataDir, cacheDir, new F.OutputKind.Device(null), Distribution: Distribution);
#if JF_FFI_BUNDLED_PLUGINS
        config = config with { BundledPlugins = [.. bundledPlugins] };
#else
        _ = bundledPlugins; // the binding has no CoreConfig.BundledPlugins yet
#endif
        return config;
    }

    /// <summary>The native library's version (About).</summary>
    public static string LibraryVersion() => F.JellyfeshMethods.LibraryVersion();

    public ulong Send(CoreCommand command) => _core.Send(FfiMapping.ToFfi(command));

    public CoreSnapshot Snapshot() => Query(() => FfiMapping.ToModel(_core.Snapshot()));

    public void SetListener(Action<CoreEvent>? listener) =>
        _core.SetListener(listener is null ? null : new F.DelegateListener(e => listener(FfiMapping.ToModel(e))));

    public IReadOnlyList<Album> Albums(uint offset, uint limit) =>
        Query(() => _core.Albums(offset, limit).Select(FfiMapping.ToModel).ToArray());

    public IReadOnlyList<Album> AlbumsSorted(AlbumSort sort, bool descending, uint offset, uint limit) =>
        Query(() => _core.AlbumsSorted(FfiMapping.ToFfi(sort), descending, offset, limit).Select(FfiMapping.ToModel).ToArray());

    public IReadOnlyList<Artist> Artists(uint offset, uint limit) =>
        Query(() => _core.Artists(offset, limit).Select(FfiMapping.ToModel).ToArray());

    public IReadOnlyList<Track> Tracks(TrackQuery query) =>
        Query(() => _core.Tracks(FfiMapping.ToFfi(query)).Select(FfiMapping.ToModel).ToArray());

    public Track? Track(long id) => Query(() => _core.Track(id) is { } t ? FfiMapping.ToModel(t) : null);

    public IReadOnlyList<Track> AlbumTracks(long albumId) =>
        Query(() => _core.AlbumTracks(albumId).Select(FfiMapping.ToModel).ToArray());

    public IReadOnlyList<Album> ArtistAlbums(long artistId) =>
        Query(() => _core.ArtistAlbums(artistId).Select(FfiMapping.ToModel).ToArray());

    public SearchResults SearchLibrary(string text, uint limit) =>
        Query(() => FfiMapping.ToModel(_core.SearchLibrary(text, limit)));

    public IReadOnlyList<TrackRef> TracksForQuery(string text, uint limit) =>
        Query(() => _core.TracksForQuery(text, limit).Select(FfiMapping.ToModel).ToArray());

    public IReadOnlyList<Track> RecentlyAdded(uint limit) =>
        Query(() => _core.RecentlyAdded(limit).Select(FfiMapping.ToModel).ToArray());

    public IReadOnlyList<TrackRow> RecentlyPlayed(uint limit) =>
        Query(() => _core.RecentlyPlayed(limit).Select(FfiMapping.ToModel).ToArray());

    public IReadOnlyList<TrackRow> MostPlayed(uint limit) =>
        Query(() => _core.MostPlayed(limit).Select(FfiMapping.ToModel).ToArray());

    public IReadOnlyList<PlaylistSummary> Playlists() =>
        Query(() => _core.Playlists().Select(FfiMapping.ToModel).ToArray());

    public PlaylistSummary? Playlist(long id) =>
        Query(() => _core.Playlist(id) is { } p ? FfiMapping.ToModel(p) : null);

    public IReadOnlyList<PlaylistEntry> PlaylistEntries(long id, uint offset, uint limit) =>
        Query(() => _core.PlaylistEntries(id, offset, limit).Select(FfiMapping.ToModel).ToArray());

    public void ExportPlaylist(long id, string path) => Query(() =>
    {
        _core.ExportPlaylist(id, path);
        return true;
    });

    public IReadOnlyList<TrackRow> Favorites(uint offset, uint limit) =>
        Query(() => _core.Favorites(offset, limit).Select(FfiMapping.ToModel).ToArray());

    public bool IsFavorite(TrackRef track) => Query(() => _core.IsFavorite(FfiMapping.ToFfi(track)));

    public string? ArtworkThumbnail(string hash, uint sizePx) => Query(() => _core.ArtworkThumbnail(hash, sizePx));

    public IReadOnlyList<QueueItem> QueueWindow(uint offset, uint limit) =>
        Query(() => _core.QueueWindow(offset, limit).Select(FfiMapping.ToModel).ToArray());

    public IReadOnlyList<HistoryEntry> History() =>
        Query(() => _core.History().Select(FfiMapping.ToModel).ToArray());

    public IReadOnlyList<string> Roots() => Query(() => _core.Roots().ToArray());

    public LibraryStats Stats() => Query(() => FfiMapping.ToModel(_core.Stats()));

    public IReadOnlyList<DeviceInfo> OutputDevices() =>
        Query(() => F.JellyfeshMethods.OutputDevices().Select(FfiMapping.ToModel).ToArray());

    public IReadOnlyList<EqPreset> EqPresets() =>
        Query(() => F.JellyfeshMethods.EqPresets().Select(FfiMapping.ToModel).ToArray());

    public string? SourceArtwork(string url, uint sizePx) => Query(() => _core.SourceArtwork(url, sizePx));

    public string? LocalizedSettingsSchema(string plugin, string language) =>
        Query(() => _core.LocalizedSettingsSchema(plugin, language));

    public string DownloadPreview(string template) => Query(() => F.JellyfeshMethods.DownloadPreview(template));

    public void SetStoreEntitlement(StoreEntitlement entitlement) => Query(() =>
    {
        _core.SetStoreEntitlement(FfiMapping.ToFfi(entitlement));
        return true;
    });

    public ulong MonotonicNowNs() => F.JellyfeshMethods.MonotonicNowNs();

    public void Shutdown() => _core.Shutdown();

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        _core.Dispose();
    }

    internal static CoreQueryException Map(F.CoreFfiException e) => new(
        e.Kind switch
        {
            F.CoreFfiErrorKind.NotAvailable => CoreQueryError.NotAvailable,
            F.CoreFfiErrorKind.ShutDown => CoreQueryError.ShutDown,
            F.CoreFfiErrorKind.InvalidArgument => CoreQueryError.InvalidArgument,
            _ => CoreQueryError.Core,
        },
        e.Message,
        e);

    private static T Query<T>(Func<T> call)
    {
        try
        {
            return call();
        }
        catch (F.CoreFfiException e)
        {
            throw Map(e);
        }
        catch (System.Text.Json.JsonException e)
        {
            throw new CoreQueryException(CoreQueryError.InvalidArgument, e.Message, e);
        }
    }
}
