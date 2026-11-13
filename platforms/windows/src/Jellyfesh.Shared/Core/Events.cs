namespace Jellyfesh.Core;

/// <summary>What the core tells the shell (jf-core <c>Event</c>). Slices arrive whole.</summary>
public abstract record CoreEvent
{
    private CoreEvent() { }

    public sealed record Playback(PlaybackState State) : CoreEvent;

    public sealed record Queue(QueueState State) : CoreEvent;

    public sealed record NowPlaying(NowPlayingInfo? Info) : CoreEvent;

    public sealed record Lyrics(LyricsState State) : CoreEvent;

    public sealed record Theme(ThemeState State) : CoreEvent;

    public sealed record Scan(ScanStatus Status) : CoreEvent;

    /// <summary>The catalogue, a playlist, a favourite or a play count changed: query lists again.</summary>
    public sealed record LibraryChanged(ulong Revision) : CoreEvent;

    /// <summary>CreatePlaylist or ImportPlaylist (the command with id <paramref name="Request"/>) made playlist <paramref name="Id"/>.</summary>
    public sealed record PlaylistCreated(ulong Request, long Id) : CoreEvent;

    public sealed record Settings(AppSettings Value) : CoreEvent;

    public sealed record Devices(DevicesState State) : CoreEvent;

    public sealed record Sources(SourcesState State) : CoreEvent;

    public sealed record Auth(AuthUpdate Update) : CoreEvent;

    public sealed record SearchResults(SearchUpdate Update) : CoreEvent;

    public sealed record BrowsePage(BrowseUpdate Update) : CoreEvent;

    /// <summary>A source is buffering the current track (rate-0 anchors meanwhile).</summary>
    public sealed record Buffering(ulong Uid, bool IsBuffering) : CoreEvent;

    public sealed record CommandFailed(ulong Id, string Message) : CoreEvent;

    /// <summary>A problem not tied to a command.</summary>
    public sealed record Error(string Message) : CoreEvent;
}
