namespace Jellyfesh.Core;

/// <summary>
/// Everything the shell can ask the core to do (jf-core <c>Command</c>). Commands are
/// fire-and-forget; a failure comes back as <see cref="CoreEvent.CommandFailed"/> with the id
/// <see cref="ICoreClient.Send"/> returned.
/// </summary>
public abstract record CoreCommand
{
    private CoreCommand() { }

    // ---- transport ----
    public sealed record Play : CoreCommand;

    public sealed record Pause : CoreCommand;

    public sealed record TogglePlayPause : CoreCommand;

    public sealed record Stop : CoreCommand;

    public sealed record Next : CoreCommand;

    /// <summary>Restarts the track after its first three seconds, otherwise goes back.</summary>
    public sealed record Previous : CoreCommand;

    public sealed record Seek(ulong PositionMs) : CoreCommand;

    /// <summary>Linear volume, 0 to 1.</summary>
    public sealed record SetVolume(float Volume) : CoreCommand;

    public sealed record SetMuted(bool Muted) : CoreCommand;

    // ---- queue ----
    public sealed record PlayTracks(IReadOnlyList<TrackRef> Tracks, uint Start) : CoreCommand;

    public sealed record Enqueue(IReadOnlyList<TrackRef> Tracks, bool PlayNext) : CoreCommand;

    public sealed record PlayCollection(Collection Collection, uint Start, bool Shuffle) : CoreCommand;

    public sealed record EnqueueCollection(Collection Collection, bool PlayNext) : CoreCommand;

    public sealed record RemoveFromQueue(ulong Uid) : CoreCommand;

    /// <summary>Moves the entry to index <paramref name="To"/> of the whole queue in playback order.</summary>
    public sealed record MoveInQueue(ulong Uid, uint To) : CoreCommand;

    public sealed record JumpTo(ulong Uid) : CoreCommand;

    public sealed record ClearQueue : CoreCommand;

    public sealed record SetShuffle(bool On) : CoreCommand;

    public sealed record SetRepeat(RepeatMode Mode) : CoreCommand;

    // ---- settings and output ----
    public sealed record SetSettings(AppSettings Settings) : CoreCommand;

    public sealed record SetEq(EqSettings Eq) : CoreCommand;

    public sealed record SetReplayGain(ReplayGainMode Mode, float PreampDb) : CoreCommand;

    public sealed record SetCrossfade(CrossfadeSettings Crossfade) : CoreCommand;

    /// <summary>A device id (or name) from the Devices slice; null follows the system default.</summary>
    public sealed record SetOutputDevice(string? Name) : CoreCommand;

    /// <summary>Sent on OS device notifications; the core never polls.</summary>
    public sealed record RefreshDevices : CoreCommand;

    // ---- library ----
    public sealed record AddRoot(string Path) : CoreCommand;

    public sealed record RemoveRoot(string Path) : CoreCommand;

    public sealed record Rescan : CoreCommand;

    /// <summary>A new playlist; <see cref="CoreEvent.PlaylistCreated"/> names it.</summary>
    public sealed record CreatePlaylist(string Name, IReadOnlyList<TrackRef> Tracks) : CoreCommand;

    public sealed record RenamePlaylist(long Id, string Name) : CoreCommand;

    public sealed record DeletePlaylist(long Id) : CoreCommand;

    public sealed record AddToPlaylist(long Id, IReadOnlyList<TrackRef> Tracks) : CoreCommand;

    public sealed record RemoveFromPlaylist(long Id, IReadOnlyList<long> Entries) : CoreCommand;

    public sealed record MovePlaylistEntry(long Id, long Entry, uint To) : CoreCommand;

    /// <summary>Imports an M3U/M3U8 file as a new playlist (export is <see cref="ICoreClient.ExportPlaylist"/>).</summary>
    public sealed record ImportPlaylist(string Path) : CoreCommand;

    public sealed record SetFavorite(TrackRef Track, bool Favorite) : CoreCommand;

    // ---- sources ----
    public sealed record InstallPlugin(string Path) : CoreCommand;

    public sealed record UninstallPlugin(string Id) : CoreCommand;

    public sealed record SetPluginEnabled(string Id, bool Enabled) : CoreCommand;

    public sealed record SetPluginSettings(string Id, string SettingsJson) : CoreCommand;

    /// <summary>The person read and accepted the community disclosure (ADR-0012).</summary>
    public sealed record AcknowledgeUnofficial(string Id) : CoreCommand;

    public sealed record BeginLogin(string Plugin, AuthMethod Method) : CoreCommand;

    /// <summary>Answers an ask-fields step. Secrets pass through once and are never stored (ADR-0015).</summary>
    public sealed record SubmitLogin(string Plugin, ulong Flow, IReadOnlyList<KeyValuePair<string, string>> Fields) : CoreCommand;

    public sealed record CancelLogin(string Plugin, ulong Flow) : CoreCommand;

    public sealed record Logout(string Plugin) : CoreCommand;

    public sealed record Search(string Query, SearchScope Scope) : CoreCommand;

    public sealed record Browse(string Plugin, string? Node, uint Page) : CoreCommand;
}
