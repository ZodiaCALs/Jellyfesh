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

    /// <summary>The remote session changed (ADR-0018).</summary>
    public sealed record Remote(RemoteState State) : CoreEvent;

    public sealed record Auth(AuthUpdate Update) : CoreEvent;

    public sealed record SearchResults(SearchUpdate Update) : CoreEvent;

    public sealed record BrowsePage(BrowseUpdate Update) : CoreEvent;

    /// <summary>A source is buffering the current track (rate-0 anchors meanwhile).</summary>
    public sealed record Buffering(ulong Uid, bool IsBuffering) : CoreEvent;

    public sealed record CommandFailed(ulong Id, string Message) : CoreEvent;

    /// <summary>A problem not tied to a command.</summary>
    public sealed record Error(string Message) : CoreEvent;

    public sealed record AiLyrics(AiLyricsState State) : CoreEvent;

    public sealed record Look(LookState State) : CoreEvent;

    public sealed record Pro(ProState State) : CoreEvent;

    /// <summary>The Update slice changed (the core's <c>Update</c>; Update is a member of Auth and others here).</summary>
    public sealed record UpdateStatus(UpdateState State) : CoreEvent;

    /// <summary>A Discover page, for the <see cref="CoreCommand.Discover"/> with its request id.</summary>
    public sealed record Discover(DiscoverPage Page) : CoreEvent;

    /// <summary>An item's details, for the <see cref="CoreCommand.Details"/> with its request id.</summary>
    public sealed record Details(DetailsPage Page) : CoreEvent;

    /// <summary>A page of comments, for the <see cref="CoreCommand.Comments"/> with its request id.</summary>
    public sealed record Comments(CommentsPage Page) : CoreEvent;

    /// <summary>A source picture asked for with <see cref="ICoreClient.SourceArtwork"/> is cached, or cannot be had.</summary>
    public sealed record Artwork(ArtworkReady Ready) : CoreEvent;
}
