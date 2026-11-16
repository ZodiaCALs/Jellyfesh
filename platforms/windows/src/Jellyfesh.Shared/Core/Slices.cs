namespace Jellyfesh.Core;

/// <summary>The <c>Playback</c> slice.</summary>
public sealed record PlaybackState(
    ulong Version,
    PlaybackStatus Status,
    PlaybackAnchor Anchor,
    float Volume,
    EqSettings Eq,
    ReplayGainMode ReplayGainMode,
    float PreampDb)
{
    public static PlaybackState Initial { get; } =
        new(0, PlaybackStatus.Stopped, default, 1f, EqSettings.None, ReplayGainMode.Off, 0f);
}

/// <summary>The <c>Queue</c> slice: the current entry and the next entries (at most 50).</summary>
public sealed record QueueState(
    ulong Version,
    uint Length,
    uint? Position,
    QueueItem? Current,
    bool Shuffle,
    RepeatMode Repeat,
    IReadOnlyList<QueueItem> UpNext)
{
    public static QueueState Initial { get; } = new(0, 0, null, null, false, RepeatMode.Off, []);
}

/// <summary>What is playing, with display metadata.</summary>
/// <param name="Link">Opens the song in its service (Open in {Service}), for songs from a source
/// that declares links.</param>
public sealed record NowPlayingInfo(
    ulong Uid,
    TrackRef Track,
    string? Path,
    string Title,
    string? Artist,
    string? Album,
    ulong? DurationMs,
    ArtworkRef? Artwork,
    TrackFormat? Format,
    string? Link = null);

public enum LyricTiming
{
    Unsynced,
    Line,
    Word,
}

public enum TextDirection
{
    Ltr,
    Rtl,
    Auto,
}

public sealed record LyricWord(uint StartMs, uint EndMs, string Text, bool JoinsNext);

public sealed record LyricLine(
    uint Id,
    uint? StartMs,
    uint? EndMs,
    string? Agent,
    string Text,
    IReadOnlyList<LyricWord> Words,
    IReadOnlyList<LyricWord> Background,
    TextDirection Direction);

public enum AuxKind
{
    Translation,
    Romanization,
}

public sealed record AuxLine(string Text, IReadOnlyList<LyricWord> Words);

/// <summary>A translation or romanization track, keyed by <see cref="LyricLine.Id"/>.</summary>
public sealed record AuxTrack(AuxKind Kind, string? Language, IReadOnlyDictionary<uint, AuxLine> Lines);

public sealed record LyricDocument(
    string? Language,
    LyricTiming Timing,
    int OffsetMs,
    IReadOnlyList<LyricLine> Lines,
    IReadOnlyList<AuxTrack> Tracks,
    string Format,
    string? Origin)
{
    public bool IsSynced => Timing != LyricTiming.Unsynced;

    public AuxTrack? Track(AuxKind kind) => Tracks.FirstOrDefault(t => t.Kind == kind);
}

/// <summary>
/// The flat arrays the lyrics cursor searches (jf-lyrics <c>LyricTimeline</c>); empty when unsynced.
/// </summary>
public sealed record LyricTimelineData(
    uint[] LineStarts,
    uint[] LineEnds,
    uint[] SegmentStarts,
    uint[] SegmentLines,
    uint[] WordOffsets,
    uint[] WordStarts,
    uint[] WordEnds)
{
    public static LyricTimelineData Empty { get; } = new([], [], [], [], [], [], []);
}

public abstract record LyricsOrigin
{
    private LyricsOrigin() { }

    public sealed record Sidecar(string Path) : LyricsOrigin;

    public sealed record Embedded : LyricsOrigin;
}

/// <summary>The <c>Lyrics</c> slice for the track with <see cref="Uid"/>.</summary>
public sealed record LyricsState(ulong Uid, LyricDocument? Doc, LyricTimelineData? Timeline, LyricsOrigin? Source);

public enum Appearance
{
    Light,
    Dark,
}

/// <summary>Every theme colour as sRGB with alpha, keyed by token name (<c>ink.ink-90</c>).</summary>
public sealed record ThemePalette(float Hue, Appearance Appearance, IReadOnlyDictionary<string, Rgba> Colors)
{
    public Rgba? this[string name] => Colors.TryGetValue(name, out var c) ? c : null;
}

/// <summary>An 8-bit sRGB colour with alpha.</summary>
public readonly record struct Rgba(byte R, byte G, byte B, byte A)
{
    /// <summary>From 0xRRGGBBAA, the packing assumed for the bindings' <c>ThemeColor.Rgba</c>.</summary>
    public static Rgba FromPacked(uint rrggbbaa) =>
        new((byte)(rrggbbaa >> 24), (byte)(rrggbbaa >> 16), (byte)(rrggbbaa >> 8), (byte)rrggbbaa);

    public uint Packed => ((uint)R << 24) | ((uint)G << 16) | ((uint)B << 8) | A;

    public override string ToString() => $"#{R:x2}{G:x2}{B:x2}{A:x2}";

    /// <summary>Linear interpolation per channel, for colour crossfades.</summary>
    public static Rgba Lerp(Rgba a, Rgba b, double t)
    {
        t = Math.Clamp(t, 0, 1);
        static byte Mix(byte x, byte y, double t) => (byte)Math.Round(x + ((y - x) * t));
        return new Rgba(Mix(a.R, b.R, t), Mix(a.G, b.G, t), Mix(a.B, b.B, t), Mix(a.A, b.A, t));
    }
}

/// <summary>The <c>Theme</c> slice: Misty colours at the artwork hue and baked backdrops.</summary>
public sealed record ThemeState(
    ulong? Uid,
    float Hue,
    bool FromArtwork,
    ThemePalette Light,
    ThemePalette Dark,
    string? BackdropLight,
    string? BackdropDark)
{
    public ThemePalette For(Appearance appearance) => appearance == Appearance.Dark ? Dark : Light;

    public string? BackdropFor(Appearance appearance) => appearance == Appearance.Dark ? BackdropDark : BackdropLight;
}

/// <summary>The <c>Devices</c> slice; <see cref="Selected"/> null follows the system default.</summary>
public sealed record DevicesState(ulong Version, IReadOnlyList<DeviceInfo> Devices, string? Selected)
{
    public static DevicesState Initial { get; } = new(0, [], null);
}

/// <summary>The <c>Sources</c> slice: the installed plugins and the services Add a Source offers, in order.</summary>
public sealed record SourcesState(ulong Version, IReadOnlyList<PluginInfo> Plugins, IReadOnlyList<SourcePreset> Presets, bool AllowCommunity)
{
    public static SourcesState Initial { get; } = new(0, [], [], false);
}

/// <summary>
/// The <c>Remote</c> slice (ADR-0018): the remote session while Jellyfesh controls a service's own
/// player. While <see cref="Source"/> is set, the player shows its track instead of the engine's,
/// and the transport commands control the service (the core routes them).
/// </summary>
/// <param name="SourceName">The source's name: Open in {Service}, and the Lyrics and Queue panes.</param>
/// <param name="Item">The track playing, as the service reports it.</param>
/// <param name="Context">The album or playlist it plays from, when the service says.</param>
/// <param name="Anchor">The time base for progress, like the Playback slice's.</param>
/// <param name="Device">The device playing: Playing on {Device}.</param>
/// <param name="Devices">Every device the service lists, for Devices.</param>
/// <param name="Error">What went wrong last talking to the service; after the session ended, why it ended.</param>
public sealed record RemoteState(
    ulong Version,
    string? Source,
    string? SourceName,
    PlaybackStatus Status,
    SourceItem? Item,
    SourceItem? Context,
    PlaybackAnchor Anchor,
    bool Shuffle,
    RepeatMode Repeat,
    RemoteDevice? Device,
    IReadOnlyList<RemoteDevice> Devices,
    string? Error)
{
    public static RemoteState Initial { get; } =
        new(0, null, null, PlaybackStatus.Stopped, null, null, default, false, RepeatMode.Off, null, [], null);

    /// <summary>A remote session runs: the player shows it.</summary>
    public bool HasSession => Source is not null;
}

/// <summary>Everything a UI mirrors; each slice carries its own version where the core has one.</summary>
public sealed record CoreSnapshot(
    PlaybackState Playback,
    QueueState Queue,
    NowPlayingInfo? NowPlaying,
    LyricsState? Lyrics,
    ThemeState? Theme,
    ulong LibraryRevision,
    bool Scanning,
    AppSettings Settings,
    DevicesState Devices,
    SourcesState Sources,
    bool Muted,
    RemoteState Remote)
{
    public static CoreSnapshot Initial { get; } = new(
        PlaybackState.Initial, QueueState.Initial, null, null, null, 0, false,
        AppSettings.Default, DevicesState.Initial, SourcesState.Initial, false, RemoteState.Initial);
}
