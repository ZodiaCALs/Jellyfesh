// P/Invoke declarations for jf-ffi's C ABI (core/jf-ffi/src/c_api.rs). LibraryImport generates
// the marshalling at compile time, so this works under NativeAOT.

using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace Jellyfesh.Ffi;

internal static unsafe partial class Native
{
    /// <summary>jf_ffi.dll, libjf_ffi.so or libjf_ffi.dylib, next to the app.</summary>
    internal const string Library = "jf_ffi";

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial CoreHandle jf_core_new(string configJson, out nint errorJson);

    /// <summary>
    /// jf_core_new with the app's secret store: the callback receives (user data, request JSON,
    /// reply function, reply context) and answers through the reply function before it returns.
    /// </summary>
    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial CoreHandle jf_core_new_with_secret_store(
        string configJson,
        delegate* unmanaged[Cdecl]<nint, nint, delegate* unmanaged[Cdecl]<nint, nint, void>, nint, void> secrets,
        nint secretsUserData,
        out nint errorJson);

    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial void jf_core_free(nint core);

    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial void jf_core_shutdown(CoreHandle core);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial ulong jf_core_send(CoreHandle core, string commandJson);

    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_snapshot(CoreHandle core);

    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial void jf_core_set_listener(
        CoreHandle core,
        delegate* unmanaged[Cdecl]<nint, nint, void> callback,
        nint userData);

    // ---- queries: (handle, arguments JSON) -> {"ok"} or {"error"} --------------------------

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_albums(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_albums_sorted(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_artists(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_tracks(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_track(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_album_tracks(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_artist_albums(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_search_library(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_tracks_for_query(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_recently_added(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_recently_played(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_most_played(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_playlists(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_playlist(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_playlist_entries(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_export_playlist(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_favorites(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_is_favorite(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_artwork_thumbnail(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_queue_window(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_history(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_roots(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_stats(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_set_store_entitlement(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_localized_settings_schema(CoreHandle core, string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_core_source_artwork(CoreHandle core, string argsJson);

    // ---- free functions ----------------------------------------------------------------------

    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_output_devices();

    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_eq_presets();

    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial ulong jf_monotonic_now_ns();

    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_default_data_dir();

    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_default_cache_dir();

    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_library_version();

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_lyrics_bar(string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_address_is_local(string argsJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint jf_download_preview(string argsJson);

    [LibraryImport(Library)]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial void jf_string_free(nint s);

    /// <summary>Copies a string the library returned and frees it; null for NULL.</summary>
    internal static string? TakeString(nint s)
    {
        if (s == 0)
        {
            return null;
        }
        try
        {
            return Marshal.PtrToStringUTF8(s);
        }
        finally
        {
            jf_string_free(s);
        }
    }
}

/// <summary>The core handle: jf_core_free runs when it is released, then the listener's GC handle is freed.</summary>
internal sealed class CoreHandle : SafeHandle
{
    private readonly object _gate = new();
    private GCHandle _listener;
    private GCHandle _secrets;
    private bool _released;

    public CoreHandle()
        : base(IntPtr.Zero, ownsHandle: true)
    {
    }

    public override bool IsInvalid => handle == IntPtr.Zero;

    /// <summary>
    /// Keeps the listener's GC handle and frees the previous one: jf_core_set_listener has
    /// returned, so the core no longer calls it.
    /// </summary>
    internal void ReplaceListener(GCHandle listener)
    {
        lock (_gate)
        {
            GCHandle old = _listener;
            _listener = _released ? default : listener;
            if (old.IsAllocated)
            {
                old.Free();
            }
            if (_released && listener.IsAllocated)
            {
                listener.Free();
            }
        }
    }

    /// <summary>Keeps the secret store's GC handle until the core is freed.</summary>
    internal void KeepSecretStore(GCHandle secrets)
    {
        lock (_gate)
        {
            _secrets = secrets;
        }
    }

    protected override bool ReleaseHandle()
    {
        // Shuts the core down and joins its event thread; once it returns, neither the listener
        // nor the secret store is called again.
        Native.jf_core_free(handle);
        lock (_gate)
        {
            _released = true;
            if (_listener.IsAllocated)
            {
                _listener.Free();
            }
            _listener = default;
            if (_secrets.IsAllocated)
            {
                _secrets.Free();
            }
            _secrets = default;
        }
        return true;
    }
}
