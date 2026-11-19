// Discover, details and comments (ADR-0022), source pictures through the core (§5) and what
// plays (docs/core-api.md, "Playing and showing quality").

namespace Jellyfesh.Core;

/// <summary>An item to ask details or comments for.</summary>
public abstract record ItemRef
{
    private ItemRef() { }

    /// <summary>A library track.</summary>
    public sealed record Track(long Id) : ItemRef;

    /// <summary>A library album.</summary>
    public sealed record Album(long Id) : ItemRef;

    /// <summary>A library artist.</summary>
    public sealed record Artist(long Id) : ItemRef;

    /// <summary>An audio file outside the library.</summary>
    public sealed record File(string Path) : ItemRef;

    /// <summary>An item a source listed: its <see cref="SourceItem.Source"/>, kind and id.</summary>
    public sealed record Source(string Plugin, ItemKind Kind, string Id) : ItemRef;

    /// <summary>The item a track reference names, when it can have details.</summary>
    public static ItemRef? Of(TrackRef track) => track switch
    {
        TrackRef.Library l => new Track(l.Id),
        TrackRef.File f => new File(f.Path),
        TrackRef.Source s => new Source(s.Track.Plugin, ItemKind.Track, s.Track.Id),
        _ => null,
    };
}

/// <summary>A song to make lyrics for.</summary>
public abstract record SongRef
{
    private SongRef() { }

    /// <summary>The song playing now.</summary>
    public sealed record Current : SongRef;

    /// <summary>A library song.</summary>
    public sealed record Library(long Id) : SongRef;
}

/// <summary>How good what plays is, as the core judges it (the app words it).</summary>
public enum QualityLabel
{
    HiResLossless,
    Lossless,
    High,
    Standard,
}

/// <summary>The format of what plays, or of a file.</summary>
public sealed record AudioFormat(
    string? Codec,
    uint? SampleRateHz,
    uint? BitDepth,
    uint? Channels,
    uint? BitrateKbps,
    bool? Lossless,
    QualityLabel? Quality);

/// <summary>ReplayGain values: gains in dB, peaks as linear amplitude.</summary>
public sealed record ReplayGainValues(float? TrackGainDb, float? TrackPeak, float? AlbumGainDb, float? AlbumPeak);

/// <summary>Technical information about a file or a stream.</summary>
public sealed record TechnicalInfo(
    AudioFormat Format,
    string? Container,
    ulong? FileSizeBytes,
    ulong? DurationMs,
    ReplayGainValues? ReplayGain,
    string? Path);

/// <summary>What a fact is; the app words every kind but <see cref="Other"/>, which brings its label.</summary>
public enum FactKind
{
    Version,
    Genre,
    Language,
    Released,
    Label,
    Composer,
    Lyricist,
    Arranger,
    Producer,
    Popularity,
    Quality,
    Isrc,
    Disc,
    Track,
    Other,
}

/// <summary>One fact about an item; <paramref name="Label"/> is the service's own, for <see cref="FactKind.Other"/>.</summary>
public sealed record Fact(FactKind Kind, string Value, string? Label = null);

/// <summary>An item's details (<see cref="CoreCommand.Details"/>).</summary>
/// <param name="Subtitle">The artist.</param>
/// <param name="Description">Album notes, an artist's biography.</param>
/// <param name="Image">A large cover or an artist photo: a file, or a URL for SourceArtwork.</param>
public sealed record DetailsPage(
    ulong Request,
    ItemRef Item,
    string Title,
    string? Subtitle,
    string? Description,
    ArtworkRef? Image,
    IReadOnlyList<Fact> Facts,
    TechnicalInfo? Technical,
    ulong? CommentCount,
    string? Attribution,
    string? Error);

/// <summary>A comment on a source item; <see cref="TimeMs"/> in ms since the epoch.</summary>
public sealed record CommentEntry(
    string Id,
    string Author,
    ArtworkRef? Avatar,
    string Text,
    ulong? Likes,
    long? TimeMs,
    ulong? Replies);

/// <summary>A page of comments; <see cref="Next"/> asks for the following one.</summary>
/// <param name="Hot">The popular comments (first page only).</param>
public sealed record CommentsPage(
    ulong Request,
    ItemRef Item,
    IReadOnlyList<CommentEntry> Hot,
    IReadOnlyList<CommentEntry> Comments,
    ulong? Total,
    string? Next,
    string? Attribution,
    string? Error);

public enum ShelfStyle
{
    /// <summary>One scrolling row of covers.</summary>
    Row,

    /// <summary>Covers that wrap.</summary>
    Grid,

    /// <summary>A numbered list.</summary>
    Chart,
}

/// <summary>One shelf of a Discover page; <see cref="More"/> is a node to browse for See All.</summary>
public sealed record Shelf(string Id, string Title, ShelfStyle Style, IReadOnlyList<SourceItem> Items, string? More);

/// <summary>A Discover page (<see cref="CoreCommand.Discover"/>); <see cref="Source"/> is "library" for the library's.</summary>
public sealed record DiscoverPage(ulong Request, string Source, IReadOnlyList<Shelf> Shelves, string? Error);

/// <summary>A source picture fetched into the cache (<see cref="Path"/>), or one that cannot be had (null).</summary>
public sealed record ArtworkReady(string Url, uint SizePx, string? Path);
