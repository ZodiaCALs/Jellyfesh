// The C# API of jf-core: the same names as the Swift and Kotlin bindings (docs/ffi.md), over
// jf-ffi's JSON C ABI.

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Text.Json.Serialization.Metadata;

namespace Jellyfesh.Ffi;

/// <summary>
/// Receives the core's events. Called on one core thread, in order: hop to the UI thread to
/// update views, and never block in OnEvent waiting for a thread that is calling SetListener,
/// Shutdown or Dispose (they wait for a call in progress). Exceptions thrown here are caught
/// and reported through <see cref="JellyfeshCore.ListenerFailed"/>.
/// </summary>
public interface CoreListener
{
    void OnEvent(Event @event);
}

/// <summary>A <see cref="CoreListener"/> that calls a delegate.</summary>
public sealed class DelegateListener(Action<Event> onEvent) : CoreListener
{
    public void OnEvent(Event @event) => onEvent(@event);
}

/// <summary>
/// Where the core keeps the sign-in tokens, API keys and cookie-jar keys of online sources:
/// implement it over the Credential Manager and pass it to
/// <see cref="JellyfeshCore(CoreConfig, SecretStore)"/>. Keys look like
/// jellyfesh/&lt;plugin-id&gt;/&lt;name&gt;; passwords never reach the store. The core calls it
/// from its own threads, possibly several at once, until the core is disposed. Throw
/// <see cref="SecretStoreException"/> (any exception counts) when the store cannot answer: if the
/// core's first call at start fails, sign-ins are kept in memory for this run and the Sources
/// slice says so (<see cref="SecretStorage.SessionOnly"/>).
/// </summary>
public interface SecretStore
{
    /// <summary>The value stored under key, or null when there is none.</summary>
    string? Get(string key);

    /// <summary>Stores value under key, replacing any earlier value.</summary>
    void Set(string key, string value);

    /// <summary>Removes key; removing a key that is not there is not an error.</summary>
    void Delete(string key);
}

/// <summary>A <see cref="SecretStore"/> call failed (jf-ffi's SecretStoreError).</summary>
public sealed class SecretStoreException(string message) : Exception(message);

/// <summary>What failed: the kinds of jf-ffi's CoreFfiError.</summary>
public enum CoreFfiErrorKind
{
    /// <summary>A library, session, audio or file error; the message says which.</summary>
    Core,

    /// <summary>The feature is not in this build yet.</summary>
    NotAvailable,

    /// <summary>The core was shut down; start a new one.</summary>
    ShutDown,

    /// <summary>An argument could not be used.</summary>
    InvalidArgument,
}

/// <summary>A core call failed (jf-ffi's CoreFfiError).</summary>
public sealed class CoreFfiException(CoreFfiErrorKind kind, string message) : Exception(message)
{
    public CoreFfiErrorKind Kind { get; } = kind;
}

/// <summary>
/// The running core: commands in, events and snapshots out, library queries on the caller's
/// thread (docs/core-api.md). Dispose it (or call Shutdown) when done; afterwards queries throw
/// <see cref="CoreFfiException"/> with <see cref="CoreFfiErrorKind.ShutDown"/>.
/// </summary>
public sealed class JellyfeshCore : IDisposable
{
    private readonly CoreHandle _handle;
    private readonly object _listenerLock = new();

    /// <summary>Opens the library and session in config.DataDir and starts the core.</summary>
    /// <exception cref="CoreFfiException">The core could not start.</exception>
    public JellyfeshCore(CoreConfig config)
    {
        ArgumentNullException.ThrowIfNull(config);
        CoreHandle handle = Native.jf_core_new(JsonSerializer.Serialize(config, JellyfeshJsonContext.Default.CoreConfig), out nint error);
        if (handle.IsInvalid)
        {
            handle.Dispose();
            throw Results.Exception(Native.TakeString(error));
        }
        _handle = handle;
    }

    /// <summary>
    /// Like <see cref="JellyfeshCore(CoreConfig)"/>, with the app's own store for the sign-ins of
    /// online sources. Without one the core uses the system keystore it reaches itself.
    /// </summary>
    /// <exception cref="CoreFfiException">The core could not start.</exception>
    public unsafe JellyfeshCore(CoreConfig config, SecretStore secrets)
    {
        ArgumentNullException.ThrowIfNull(config);
        ArgumentNullException.ThrowIfNull(secrets);
        GCHandle box = GCHandle.Alloc(secrets);
        CoreHandle handle = Native.jf_core_new_with_secret_store(
            JsonSerializer.Serialize(config, JellyfeshJsonContext.Default.CoreConfig),
            &OnSecret,
            GCHandle.ToIntPtr(box),
            out nint error);
        if (handle.IsInvalid)
        {
            handle.Dispose();
            box.Free();
            throw Results.Exception(Native.TakeString(error));
        }
        handle.KeepSecretStore(box);
        _handle = handle;
    }

    /// <summary>Raised (on the core's event thread) when a listener throws.</summary>
    public static event Action<Exception>? ListenerFailed;

    /// <summary>
    /// Queues a command and returns its id; a failure comes back as Event.CommandFailed with
    /// that id. Returns 0 after Shutdown.
    /// </summary>
    public ulong Send(Command command)
    {
        ArgumentNullException.ThrowIfNull(command);
        return Native.jf_core_send(_handle, JsonSerializer.Serialize(command, JellyfeshJsonContext.Default.Command));
    }

    /// <summary>The current state of every slice (the last state after Shutdown).</summary>
    public Snapshot Snapshot()
    {
        string json = Native.TakeString(Native.jf_core_snapshot(_handle))
            ?? throw new CoreFfiException(CoreFfiErrorKind.Core, "the core returned no snapshot");
        return JsonSerializer.Deserialize(json, JellyfeshJsonContext.Default.Snapshot)!;
    }

    /// <summary>
    /// Sets the listener that receives every event from now on, replacing any earlier one; null
    /// removes it. When this returns, the earlier listener is no longer called (unless this is
    /// called from inside its OnEvent). Read Snapshot() after setting it for the state so far.
    /// </summary>
    public unsafe void SetListener(CoreListener? listener)
    {
        lock (_listenerLock)
        {
            if (listener is null)
            {
                Native.jf_core_set_listener(_handle, null, 0);
                _handle.ReplaceListener(default);
            }
            else
            {
                GCHandle box = GCHandle.Alloc(listener);
                Native.jf_core_set_listener(_handle, &OnEvent, GCHandle.ToIntPtr(box));
                _handle.ReplaceListener(box);
            }
        }
    }

    public IReadOnlyList<Album> Albums(uint offset, uint limit) =>
        Query(Native.jf_core_albums, Args(new PageArgs(offset, limit), InteropJsonContext.Default.PageArgs), JellyfeshJsonContext.Default.IReadOnlyListAlbum)!;

    /// <summary>A page of albums in the given order.</summary>
    public IReadOnlyList<Album> AlbumsSorted(AlbumSort sort, bool descending, uint offset, uint limit) =>
        Query(Native.jf_core_albums_sorted, Args(new SortedPageArgs(sort, descending, offset, limit), InteropJsonContext.Default.SortedPageArgs), JellyfeshJsonContext.Default.IReadOnlyListAlbum)!;

    public IReadOnlyList<Artist> Artists(uint offset, uint limit) =>
        Query(Native.jf_core_artists, Args(new PageArgs(offset, limit), InteropJsonContext.Default.PageArgs), JellyfeshJsonContext.Default.IReadOnlyListArtist)!;

    public IReadOnlyList<Track> Tracks(TrackQuery query) =>
        Query(Native.jf_core_tracks, Args(new QueryArgs(query), InteropJsonContext.Default.QueryArgs), JellyfeshJsonContext.Default.IReadOnlyListTrack)!;

    /// <summary>One library track by id, or null.</summary>
    public Track? Track(long id) =>
        Query(Native.jf_core_track, Args(new IdArgs(id), InteropJsonContext.Default.IdArgs), JellyfeshJsonContext.Default.Track);

    /// <summary>The tracks of an album, by disc and track number.</summary>
    public IReadOnlyList<Track> AlbumTracks(long albumId) =>
        Query(Native.jf_core_album_tracks, Args(new AlbumIdArgs(albumId), InteropJsonContext.Default.AlbumIdArgs), JellyfeshJsonContext.Default.IReadOnlyListTrack)!;

    /// <summary>The albums an artist appears on, by year then title.</summary>
    public IReadOnlyList<Album> ArtistAlbums(long artistId) =>
        Query(Native.jf_core_artist_albums, Args(new ArtistIdArgs(artistId), InteropJsonContext.Default.ArtistIdArgs), JellyfeshJsonContext.Default.IReadOnlyListAlbum)!;

    /// <summary>Searches the library only (fast, synchronous). Command.Search searches the sources too.</summary>
    public SearchResults SearchLibrary(string text, uint limit) =>
        Query(Native.jf_core_search_library, Args(new TextArgs(text, limit), InteropJsonContext.Default.TextArgs), JellyfeshJsonContext.Default.SearchResults)!;

    /// <summary>Library tracks to play for a search.</summary>
    public IReadOnlyList<TrackRef> TracksForQuery(string text, uint limit) =>
        Query(Native.jf_core_tracks_for_query, Args(new TextArgs(text, limit), InteropJsonContext.Default.TextArgs), JellyfeshJsonContext.Default.IReadOnlyListTrackRef)!;

    public IReadOnlyList<Track> RecentlyAdded(uint limit) =>
        Query(Native.jf_core_recently_added, Args(new LimitArgs(limit), InteropJsonContext.Default.LimitArgs), JellyfeshJsonContext.Default.IReadOnlyListTrack)!;

    /// <summary>Recently played tracks (plays that counted), newest first.</summary>
    public IReadOnlyList<TrackRow> RecentlyPlayed(uint limit) =>
        Query(Native.jf_core_recently_played, Args(new LimitArgs(limit), InteropJsonContext.Default.LimitArgs), JellyfeshJsonContext.Default.IReadOnlyListTrackRow)!;

    /// <summary>The most played tracks, most first.</summary>
    public IReadOnlyList<TrackRow> MostPlayed(uint limit) =>
        Query(Native.jf_core_most_played, Args(new LimitArgs(limit), InteropJsonContext.Default.LimitArgs), JellyfeshJsonContext.Default.IReadOnlyListTrackRow)!;

    /// <summary>Every playlist, by name.</summary>
    public IReadOnlyList<PlaylistSummary> Playlists() =>
        Query(Native.jf_core_playlists, NoArguments, JellyfeshJsonContext.Default.IReadOnlyListPlaylistSummary)!;

    /// <summary>One playlist, or null.</summary>
    public PlaylistSummary? Playlist(long id) =>
        Query(Native.jf_core_playlist, Args(new IdArgs(id), InteropJsonContext.Default.IdArgs), JellyfeshJsonContext.Default.PlaylistSummary);

    public IReadOnlyList<PlaylistEntry> PlaylistEntries(long id, uint offset, uint limit) =>
        Query(Native.jf_core_playlist_entries, Args(new PlaylistPageArgs(id, offset, limit), InteropJsonContext.Default.PlaylistPageArgs), JellyfeshJsonContext.Default.IReadOnlyListPlaylistEntry)!;

    /// <summary>Writes a playlist to path as M3U8, replacing the file atomically.</summary>
    public void ExportPlaylist(long id, string path) =>
        Query(Native.jf_core_export_playlist, Args(new ExportArgs(id, path), InteropJsonContext.Default.ExportArgs), JellyfeshJsonContext.Default.String);

    /// <summary>A page of favourites, the latest first.</summary>
    public IReadOnlyList<TrackRow> Favorites(uint offset, uint limit) =>
        Query(Native.jf_core_favorites, Args(new PageArgs(offset, limit), InteropJsonContext.Default.PageArgs), JellyfeshJsonContext.Default.IReadOnlyListTrackRow)!;

    public bool IsFavorite(TrackRef track) =>
        Query(Native.jf_core_is_favorite, Args(new TrackArgs(track), InteropJsonContext.Default.TrackArgs), JellyfeshJsonContext.Default.Boolean);

    /// <summary>A JPEG of the artwork with this hash at about sizePx, created on first use; null for an unknown hash.</summary>
    public string? ArtworkThumbnail(string hash, uint sizePx) =>
        Query(Native.jf_core_artwork_thumbnail, Args(new ThumbnailArgs(hash, sizePx), InteropJsonContext.Default.ThumbnailArgs), JellyfeshJsonContext.Default.String);

    /// <summary>A page of the queue in playback order.</summary>
    public IReadOnlyList<QueueItem> QueueWindow(uint offset, uint limit) =>
        Query(Native.jf_core_queue_window, Args(new PageArgs(offset, limit), InteropJsonContext.Default.PageArgs), JellyfeshJsonContext.Default.IReadOnlyListQueueItem)!;

    /// <summary>Recently played queue entries, oldest first.</summary>
    public IReadOnlyList<HistoryEntry> History() =>
        Query(Native.jf_core_history, NoArguments, JellyfeshJsonContext.Default.IReadOnlyListHistoryEntry)!;

    /// <summary>The library folders.</summary>
    public IReadOnlyList<string> Roots() =>
        Query(Native.jf_core_roots, NoArguments, JellyfeshJsonContext.Default.IReadOnlyListString)!;

    public LibraryStats Stats() =>
        Query(Native.jf_core_stats, NoArguments, JellyfeshJsonContext.Default.LibraryStats)!;

    /// <summary>Saves the session, stops playback and the listener. Further calls do nothing.</summary>
    public void Shutdown()
    {
        if (!_handle.IsClosed)
        {
            Native.jf_core_shutdown(_handle);
        }
    }

    /// <summary>Shuts the core down (if Shutdown has not) and frees it.</summary>
    public void Dispose() => _handle.Dispose();

    private const string NoArguments = "{}";

    private static string Args<T>(T args, JsonTypeInfo<T> info) => JsonSerializer.Serialize(args, info);

    private T? Query<T>(Func<CoreHandle, string, nint> call, string args, JsonTypeInfo<T> result) =>
        Results.Unwrap(Native.TakeString(call(_handle, args)), result);

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    private static unsafe void OnSecret(
        nint userData,
        nint requestJson,
        delegate* unmanaged[Cdecl]<nint, nint, void> reply,
        nint replyContext)
    {
        // Nothing may throw back into Rust: every failure becomes an error answer.
        string answer;
        try
        {
            if (GCHandle.FromIntPtr(userData).Target is not SecretStore store)
            {
                throw new SecretStoreException("the secret store is gone");
            }
            ReadOnlySpan<byte> json = MemoryMarshal.CreateReadOnlySpanFromNullTerminated((byte*)requestJson);
            SecretRequest request = JsonSerializer.Deserialize(json, InteropJsonContext.Default.SecretRequest)
                ?? throw new SecretStoreException("an empty request");
            string? value = null;
            switch (request.Op)
            {
                case "get":
                    value = store.Get(request.Key);
                    break;
                case "set":
                    store.Set(request.Key, request.Value ?? "");
                    break;
                case "delete":
                    store.Delete(request.Key);
                    break;
                default:
                    throw new SecretStoreException($"unknown request {request.Op}");
            }
            answer = JsonSerializer.Serialize(new SecretAnswer(value), InteropJsonContext.Default.SecretAnswer);
        }
        catch (Exception e)
        {
            answer = JsonSerializer.Serialize(new SecretFailure(e.Message), InteropJsonContext.Default.SecretFailure);
        }
        nint utf8 = Marshal.StringToCoTaskMemUTF8(answer);
        try
        {
            reply(replyContext, utf8);
        }
        finally
        {
            Marshal.FreeCoTaskMem(utf8);
        }
    }

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    private static unsafe void OnEvent(nint userData, nint eventJson)
    {
        // Nothing may throw back into Rust: an exception here would end the process.
        try
        {
            if (GCHandle.FromIntPtr(userData).Target is not CoreListener listener)
            {
                return;
            }
            ReadOnlySpan<byte> json = MemoryMarshal.CreateReadOnlySpanFromNullTerminated((byte*)eventJson);
            Event? ev = JsonSerializer.Deserialize(json, JellyfeshJsonContext.Default.Event);
            if (ev is not null)
            {
                listener.OnEvent(ev);
            }
        }
        catch (Exception e)
        {
            try
            {
                Trace.TraceError("Jellyfesh core listener failed: {0}", e);
                ListenerFailed?.Invoke(e);
            }
            catch (Exception)
            {
                // A failing ListenerFailed handler is ignored for the same reason.
            }
        }
    }
}

/// <summary>The core's free functions.</summary>
public static class JellyfeshMethods
{
    /// <summary>Audio output devices, for a device picker.</summary>
    public static IReadOnlyList<DeviceInfo> OutputDevices() =>
        Results.Unwrap(Native.TakeString(Native.jf_output_devices()), JellyfeshJsonContext.Default.IReadOnlyListDeviceInfo)!;

    /// <summary>Nanoseconds on the clock playback anchors are stamped with (for DisplayClock).</summary>
    public static ulong MonotonicNowNs() => Native.jf_monotonic_now_ns();

    /// <summary>The platform's default data folder (JF_DATA_DIR overrides it), or null.</summary>
    public static string? DefaultDataDir() => Native.TakeString(Native.jf_default_data_dir());

    /// <summary>The platform's default cache folder (JF_CACHE_DIR overrides it), or null.</summary>
    public static string? DefaultCacheDir() => Native.TakeString(Native.jf_default_cache_dir());

    /// <summary>The version of the native library.</summary>
    public static string LibraryVersion() => Native.TakeString(Native.jf_library_version()) ?? "";
}

/// <summary>Reads {"ok": value} or {"error": {kind, message}}.</summary>
internal static class Results
{
    internal static T? Unwrap<T>(string? json, JsonTypeInfo<T> info)
    {
        if (json is null)
        {
            throw new CoreFfiException(CoreFfiErrorKind.Core, "the core returned no result");
        }
        using JsonDocument document = JsonDocument.Parse(json);
        JsonElement root = document.RootElement;
        if (root.TryGetProperty("error", out JsonElement error))
        {
            throw Exception(error.GetRawText());
        }
        return root.GetProperty("ok").Deserialize(info);
    }

    internal static CoreFfiException Exception(string? errorJson)
    {
        ErrorJson? error = errorJson is null ? null : JsonSerializer.Deserialize(errorJson, InteropJsonContext.Default.ErrorJson);
        if (error is null)
        {
            return new CoreFfiException(CoreFfiErrorKind.Core, "the core did not say what failed");
        }
        CoreFfiErrorKind kind = Enum.TryParse(error.Kind, out CoreFfiErrorKind parsed) ? parsed : CoreFfiErrorKind.Core;
        return new CoreFfiException(kind, error.Message);
    }
}
