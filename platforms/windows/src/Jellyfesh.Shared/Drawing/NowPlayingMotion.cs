using Jellyfesh.Core;

namespace Jellyfesh.Drawing;

/// <summary>Where a lyric line is relative to the line being sung.</summary>
public enum LyricLineRole
{
    /// <summary>Not yet sung: lyrics.ink-inactive (ink-50).</summary>
    Upcoming,

    /// <summary>The line being sung: ink-90 at full size.</summary>
    Active,

    /// <summary>Already sung: ink-30.</summary>
    Sung,
}

/// <summary>One keyframe of the lyrics cascade: at <see cref="Progress"/> (0 to 1) a line is <see cref="Offset"/> below the list.</summary>
public readonly record struct CascadeKeyframe(double Progress, double Offset);

/// <summary>
/// Now Playing's motion after Apple Music (docs/design-controls.md, "v0.0.4"; Luminous, docs/ui.md,
/// v0.0.6), as numbers the views animate with the toolkit: the artwork's pause scale, lyric sizes
/// and the lyrics cascade. With reduced motion the artwork stays at full size and the lyrics jump.
/// </summary>
public static class NowPlayingMotion
{
    /// <summary>player.artwork-paused-scale.</summary>
    public const double ArtworkPausedScale = 0.88;

    /// <summary>lyrics.scale-inactive (Luminous: 92 %): every line but the active one, scaled from its start edge.</summary>
    public const double InactiveLineScale = 0.92;

    /// <summary>lyrics.scale-small.</summary>
    public const double SmallLyrics = 0.8;

    /// <summary>lyrics.scale-large.</summary>
    public const double LargeLyrics = 1.2;

    /// <summary>lyrics.stagger (Luminous: 40 ms): each following line starts this much after the one above.</summary>
    public static readonly TimeSpan Stagger = TimeSpan.FromMilliseconds(40);

    /// <summary>Lines further below the active one than this start together, so the cascade never drags.</summary>
    public const int MaxStaggeredLines = 8;

    /// <summary>The artwork's scale: 1 while playing, 0.88 when paused; always 1 with reduced motion.</summary>
    public static double ArtworkScale(bool playing, bool reduceMotion) =>
        playing || reduceMotion ? 1 : ArtworkPausedScale;

    /// <summary>The pane width at and above which lyric lines take their full token size (Luminous: 40).</summary>
    public const double FullLyricsWidth = 640;

    /// <summary>
    /// The lyric line size for a lyrics pane <paramref name="paneWidth"/> wide, before the Lyrics size
    /// setting: lyrics.line (40, the fluid maximum) from a 640 epx pane, smaller in narrower panes but
    /// never under the fluid minimum (28).
    /// </summary>
    public static double LyricLineSize(double paneWidth, double min, double max) =>
        double.IsFinite(paneWidth) && paneWidth > 0 ? Math.Clamp(max * paneWidth / FullLyricsWidth, min, max) : min;

    /// <summary>The Lyrics size setting as a factor of the token sizes (line, translation, romanization).</summary>
    public static double LyricsScale(LyricsSize size) => size switch
    {
        LyricsSize.Small => SmallLyrics,
        LyricsSize.Large => LargeLyrics,
        _ => 1,
    };

    /// <summary>The role of line <paramref name="index"/> when <paramref name="activeIndex"/> is sung (-1: none yet).</summary>
    public static LyricLineRole Role(int index, int activeIndex) =>
        index == activeIndex ? LyricLineRole.Active
        : activeIndex >= 0 && index < activeIndex ? LyricLineRole.Sung
        : LyricLineRole.Upcoming;

    /// <summary>A line's scale: full for the active line, <see cref="InactiveLineScale"/> for the others.</summary>
    public static double LineScale(LyricLineRole role) => role == LyricLineRole.Active ? 1 : InactiveLineScale;

    /// <summary>
    /// How much later than the scroll line <paramref name="index"/> starts moving: nothing for the
    /// active line and those above it, then <see cref="Stagger"/> more for each line below it.
    /// </summary>
    public static TimeSpan CascadeDelay(int index, int activeIndex) =>
        index <= activeIndex || activeIndex < 0
            ? TimeSpan.Zero
            : Stagger * Math.Min(index - activeIndex, MaxStaggeredLines);

    /// <summary>
    /// The extra offset that makes a line start <paramref name="delay"/> after a scroll of
    /// <paramref name="distance"/> that runs <paramref name="duration"/> on <paramref name="curve"/>:
    /// the scroll moves it by distance·E(t/D), the cascade wants distance·E((t−delay)/D), so the line
    /// is drawn distance·(E(t/D) − E((t−delay)/D)) lower. Sampled over the delay plus the duration
    /// (0 at both ends) for a keyframe animation the compositor runs.
    /// </summary>
    public static IReadOnlyList<CascadeKeyframe> CascadeLag(double distance, TimeSpan delay, TimeSpan duration, CubicBezierCurve curve, int samples = 12)
    {
        ArgumentNullException.ThrowIfNull(curve);
        var d = Math.Max(1, duration.TotalMilliseconds);
        var lag = Math.Max(0, delay.TotalMilliseconds);
        var total = d + lag;
        var frames = new List<CascadeKeyframe>(samples + 1);
        for (var i = 0; i <= samples; i++)
        {
            var t = total * i / samples;
            var offset = distance * (curve.Ease(t / d) - curve.Ease((t - lag) / d));
            frames.Add(new CascadeKeyframe((double)i / samples, i == samples ? 0 : offset));
        }

        return frames;
    }
}
