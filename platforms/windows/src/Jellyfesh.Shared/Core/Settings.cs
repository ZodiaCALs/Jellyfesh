namespace Jellyfesh.Core;

public enum AppearancePreference
{
    System,
    Light,
    Dark,
}

/// <summary>The Now Playing backdrop: baked artwork, or Misty's own field (ADR-0013).</summary>
public enum BackdropStyle
{
    Artwork,
    Misty,
}

/// <summary>Where the theme hue comes from.</summary>
public abstract record HueSource
{
    private HueSource() { }

    public sealed record Artwork : HueSource;

    public sealed record Fixed(float Hue) : HueSource;
}

/// <summary>The size of lyrics on Now Playing: lines, translations and romanizations scale together.</summary>
public enum LyricsSize
{
    Small,
    Medium,
    Large,
}

/// <summary>How lyrics lines line up on Now Playing.</summary>
public enum LyricsAlign
{
    /// <summary>The start edge (left in left-to-right text).</summary>
    Start,
    Center,
}

/// <summary>Crossfade between tracks; with album continuity, tracks of one album join gaplessly.</summary>
public sealed record CrossfadeSettings(bool Enabled, uint DurationMs, bool AlbumContinuity)
{
    public const uint MinDurationMs = 1_000;
    public const uint MaxDurationMs = 12_000;

    public static CrossfadeSettings Default { get; } = new(false, 6_000, true);
}

/// <summary>
/// The core's <c>Settings</c> slice. The core stores it and applies playback, library and source
/// settings; the shell applies the appearance ones. Change it by sending back a modified copy
/// (<see cref="CoreCommand.SetSettings"/>).
/// </summary>
public sealed record AppSettings(
    AppearancePreference Appearance,
    BackdropStyle Style,
    string AppIcon,
    HueSource HueSource,
    bool ReduceEffects,
    float Transparency,
    LyricsSize LyricsSize,
    LyricsAlign LyricsAlign,
    bool ShowTranslation,
    bool ShowRomanization,
    CrossfadeSettings Crossfade,
    ReplayGainMode ReplayGainMode,
    float PreampDb,
    EqSettings Eq,
    string? OutputDevice,
    bool ResumeOnLaunch,
    bool WatchFolders,
    bool AllowCommunityPlugins)
{
    /// <summary>How see-through glass is by default: the tokens' own alphas (0 solid … 1 clearest).</summary>
    public const float DefaultTransparency = 0.5f;

    public static AppSettings Default { get; } = new(
        AppearancePreference.System,
        BackdropStyle.Artwork,
        "automatic",
        new HueSource.Artwork(),
        ReduceEffects: false,
        DefaultTransparency,
        LyricsSize.Medium,
        LyricsAlign.Start,
        ShowTranslation: true,
        ShowRomanization: true,
        CrossfadeSettings.Default,
        ReplayGainMode.Off,
        PreampDb: 0f,
        EqSettings.Flat,
        OutputDevice: null,
        ResumeOnLaunch: false,
        WatchFolders: true,
        AllowCommunityPlugins: false);
}
