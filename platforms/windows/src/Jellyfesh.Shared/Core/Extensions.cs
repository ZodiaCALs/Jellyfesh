// The v0.0.7 slices: AI Lyrics (ADR-0019), the look (ADR-0020), Jellyfesh Pro and updates
// (ADR-0021), as docs/core-api.md describes them. The core decides every value; the app shows them.

namespace Jellyfesh.Core;

// ---- AI Lyrics ------------------------------------------------------------------------------

public enum AiLyricsMode
{
    /// <summary>Whisper in the app; nothing leaves the device.</summary>
    OnDevice,

    /// <summary>The person's own provider, with their API key, after consent.</summary>
    Online,
}

/// <summary>What making lyrics would do now.</summary>
/// <param name="Engine">The engine, in the plugin's words: "Whisper Base on this device".</param>
/// <param name="UploadTo">The origin the online mode sends audio to.</param>
/// <param name="Automatic">Lyrics are made for each song that starts without synced lyrics.</param>
public sealed record AiLyricsPlan(AiLyricsMode Mode, string Engine, string? UploadTo, bool Automatic);

/// <summary>What must happen before <see cref="CoreCommand.MakeLyrics"/> works.</summary>
public abstract record AiLyricsNeed
{
    private AiLyricsNeed() { }

    /// <summary>Ready: Create Synced Lyrics works.</summary>
    public sealed record Nothing : AiLyricsNeed;

    /// <summary>Jellyfesh Pro is inactive: the Pro sheet.</summary>
    public sealed record Pro : AiLyricsNeed;

    /// <summary>The online mode needs the person's API key: Open AI Lyrics Settings.</summary>
    public sealed record SignIn : AiLyricsNeed;

    /// <summary>"Send This Song to {Origin}?", answered with AllowAudioUpload.</summary>
    public sealed record Consent(string Origin) : AiLyricsNeed;

    /// <summary>Download this on-device model first.</summary>
    public sealed record Model(string Id, string Name, ulong Bytes) : AiLyricsNeed;

    /// <summary>Making lyrics cannot work as set up; the reason is the core's.</summary>
    public sealed record Unavailable(string Reason) : AiLyricsNeed;
}

/// <summary>Where a lyrics job is.</summary>
public abstract record LyricsJobStage
{
    private LyricsJobStage() { }

    public sealed record Waiting : LyricsJobStage;

    public sealed record ReadingAudio : LyricsJobStage;

    public sealed record Sending(string Origin) : LyricsJobStage;

    /// <summary><paramref name="Fraction"/> from 0 to 1.</summary>
    public sealed record Recognising(float Fraction) : LyricsJobStage;

    public sealed record TimingWords : LyricsJobStage;

    public sealed record Done : LyricsJobStage;

    public sealed record Failed(string Message) : LyricsJobStage;

    public sealed record Cancelled : LyricsJobStage;

    /// <summary>Still waiting or running: Cancel stops it.</summary>
    public bool IsActive => this is not (Done or Failed or Cancelled);
}

/// <summary>A lyrics job: <see cref="Uid"/> when made for the song playing, <see cref="LibraryId"/> for a library song.</summary>
public sealed record LyricsJob(ulong Id, string Title, string? Artist, ulong? Uid, long? LibraryId, LyricsJobStage Stage);

/// <summary>Where an on-device speech model is.</summary>
public abstract record SpeechModelStatus
{
    private SpeechModelStatus() { }

    public sealed record NotDownloaded : SpeechModelStatus;

    /// <summary><paramref name="Fraction"/> from 0 to 1.</summary>
    public sealed record Downloading(float Fraction) : SpeechModelStatus;

    public sealed record Ready : SpeechModelStatus;

    public sealed record Failed(string Message) : SpeechModelStatus;
}

/// <summary>An on-device speech model ("Base · 142 MB").</summary>
public sealed record SpeechModel(string Id, string Name, ulong Bytes, SpeechModelStatus State);

/// <summary>An origin the person allowed a plugin to send audio to (Sending Audio).</summary>
public sealed record AudioConsent(string Plugin, string Origin);

/// <summary>The <c>AiLyrics</c> slice.</summary>
/// <param name="Available">Offer Create Synced Lyrics (Pro may still be needed: see <see cref="Need"/>).</param>
/// <param name="Plugin">The AI Lyrics plugin: the first enabled one that makes lyrics.</param>
/// <param name="Plan">What making lyrics would do; null until the plugin answered or while Pro is off.</param>
/// <param name="Jobs">Waiting and running jobs and the newest finished ones, oldest first.</param>
public sealed record AiLyricsState(
    ulong Version,
    bool Available,
    string? Plugin,
    AiLyricsPlan? Plan,
    AiLyricsNeed Need,
    IReadOnlyList<LyricsJob> Jobs,
    IReadOnlyList<SpeechModel> Models,
    IReadOnlyList<AudioConsent> Consents)
{
    public static AiLyricsState Initial { get; } =
        new(0, false, null, null, new AiLyricsNeed.Unavailable(string.Empty), [], [], []);

    /// <summary>The newest job for the queue entry <paramref name="uid"/> (the song playing), if any.</summary>
    public LyricsJob? JobFor(ulong uid) => Jobs.LastOrDefault(j => j.Uid == uid);
}

// ---- the look -------------------------------------------------------------------------------

/// <summary>The face text uses: the platform's own (Segoe UI Variable, and its alternatives).</summary>
public enum Typeface
{
    System,
    Rounded,
    Serif,
    Mono,
}

/// <summary>Spacing: × 0.8, 1.0 or 1.2.</summary>
public enum Density
{
    Compact,
    Standard,
    Roomy,
}

public enum LyricsWeight
{
    Regular,
    Medium,
    Semibold,
    Bold,
    Heavy,
}

/// <summary>The colour of the sung part of lyrics.</summary>
public abstract record LyricsHighlight
{
    private LyricsHighlight() { }

    public sealed record Artwork : LyricsHighlight;

    public sealed record Accent : LyricsHighlight;

    /// <summary>0xRRGGBB.</summary>
    public sealed record Custom(uint Rgb) : LyricsHighlight;
}

/// <summary>A place of the navigation pane (Settings is never listed and always shown).</summary>
public enum Place
{
    Home,
    Albums,
    Artists,
    Songs,
    Playlists,
    Favourites,
    Search,
    Sources,
}

/// <summary>A section of Home.</summary>
public enum HomeSection
{
    Summary,
    RecentlyAdded,
    RecentlyPlayed,
    MostPlayed,
    Favourites,
}

public enum NowPlayingLayout
{
    Stage,
    Artwork,
    Lyrics,
}

public enum MiniPlayerStyle
{
    Bar,
    Compact,
    Floating,
}

/// <summary>
/// The <c>Look</c> slice: every value resolved by the core (ADR-0020 §2). With
/// <see cref="Customized"/> false every value is the Luminous default.
/// </summary>
/// <param name="CornerScale">Radii ×, 0–2.</param>
/// <param name="Glass">Glass alpha and blur ×, 0–1 (Reduce Transparency and the Transparency setting still win).</param>
/// <param name="Aurora">The aurora field's strength, 0–1.</param>
/// <param name="TextScale">Every type size ×, 0.85–1.30.</param>
/// <param name="LyricsGlow">0–1.</param>
/// <param name="Sidebar">The places shown, in order.</param>
/// <param name="Home">Home's sections, in order.</param>
/// <param name="Plugin">The appearance plugin that made it.</param>
public sealed record LookState(
    ulong Version,
    float CornerScale,
    float Glass,
    float Aurora,
    Typeface Typeface,
    float TextScale,
    Density Density,
    LyricsWeight LyricsWeight,
    float LyricsGlow,
    LyricsHighlight LyricsHighlight,
    IReadOnlyList<Place> Sidebar,
    IReadOnlyList<HomeSection> Home,
    NowPlayingLayout NowPlaying,
    MiniPlayerStyle MiniPlayer,
    bool Customized,
    string? Plugin)
{
    /// <summary>The token field.aurora-strength, the default aurora.</summary>
    public const float DefaultAurora = 0.75f;

    /// <summary>The Luminous defaults (the core's LookSnapshot::defaults).</summary>
    public static LookState Default { get; } = new(
        0,
        CornerScale: 1f,
        Glass: 1f,
        Aurora: DefaultAurora,
        Typeface.System,
        TextScale: 1f,
        Density.Standard,
        LyricsWeight.Bold,
        LyricsGlow: 0.6f,
        new LyricsHighlight.Artwork(),
        [Place.Home, Place.Albums, Place.Artists, Place.Songs, Place.Playlists, Place.Favourites, Place.Search, Place.Sources],
        [HomeSection.Summary, HomeSection.RecentlyAdded, HomeSection.RecentlyPlayed, HomeSection.MostPlayed],
        NowPlayingLayout.Stage,
        MiniPlayerStyle.Bar,
        Customized: false,
        Plugin: null);

    /// <summary>The spacing factor of <see cref="Density"/>.</summary>
    public double Spacing => Density switch
    {
        Density.Compact => 0.8,
        Density.Roomy => 1.2,
        _ => 1.0,
    };
}

// ---- Jellyfesh Pro and updates --------------------------------------------------------------

/// <summary>A store that sells Jellyfesh Pro.</summary>
public enum ProStore
{
    AppStore,
    MicrosoftStore,
    GooglePlay,
}

/// <summary>Where Pro comes from.</summary>
public abstract record ProSource
{
    private ProSource() { }

    public sealed record None : ProSource;

    /// <summary>A Pro Preview build: "Pro features are free during the public beta."</summary>
    public sealed record Preview : ProSource;

    /// <summary>"Jellyfesh Pro · Purchased".</summary>
    public sealed record Purchased(ProStore Store) : ProSource;

    /// <summary>"Jellyfesh Pro · Licence …{IdSuffix}"; issued at midnight UTC, ms since the epoch.</summary>
    public sealed record Licence(string IdSuffix, long IssuedAtMs) : ProSource;
}

/// <summary>Why a licence key was not accepted (the app words it).</summary>
public enum LicenceError
{
    Malformed,
    UnknownKey,
    BadSignature,
    WrongProduct,
    Revoked,
}

/// <summary>The <c>Pro</c> slice.</summary>
/// <param name="Preview">A Pro Preview build.</param>
/// <param name="LicenceError">Why the last key was refused.</param>
/// <param name="Stores">Where this build can sell Pro: Buy and Restore Purchases only for these.</param>
/// <param name="LicenceKeys">Offer Enter Licence Key…</param>
public sealed record ProState(
    ulong Version,
    bool Active,
    ProSource Source,
    bool Preview,
    LicenceError? LicenceError,
    IReadOnlyList<ProStore> Stores,
    bool LicenceKeys)
{
    public static ProState Initial { get; } = new(0, false, new ProSource.None(), false, null, [], true);
}

/// <summary>What a store says about Jellyfesh Pro, as the app's store glue verified it.</summary>
/// <param name="Product">The store's product id (org.jellyfesh.pro).</param>
/// <param name="Active">Whether the person owns it now (false after a refund).</param>
/// <param name="Transaction">The store's order id, if it gives one.</param>
public sealed record StoreEntitlement(ProStore Store, string Product, bool Active, string? Transaction);

/// <summary>A newer release and its page.</summary>
public sealed record UpdateInfo(string Version, string Url);

/// <summary>The <c>Update</c> slice (direct downloads).</summary>
/// <param name="Supported">Show Check for Updates and its setting.</param>
/// <param name="Auto">Checks run by themselves (supported and the setting on).</param>
public sealed record UpdateState(
    ulong Version,
    bool Supported,
    bool Checking,
    UpdateInfo? Available,
    long? LastCheckedMs,
    bool Auto,
    string? Error)
{
    public static UpdateState Initial { get; } = new(0, false, false, null, null, false, null);
}

/// <summary>
/// How the Windows app applies the Look slice (ADR-0020 §2): the navigation pane's places and
/// Home's sections in the slice's order, the platform's own faces, the aurora's strength.
/// </summary>
public static class LookRules
{
    /// <summary>The navigation pane's tags for the slice's places, in its order (Settings is always shown apart).</summary>
    public static IReadOnlyList<string> SidebarTags(LookState look) => look.Sidebar.Select(p => p switch
    {
        Place.Home => "home",
        Place.Albums => "albums",
        Place.Artists => "artists",
        Place.Songs => "songs",
        Place.Playlists => "playlists",
        Place.Favourites => "favorites",
        Place.Search => "search",
        _ => "sources",
    }).Distinct().ToArray();

    /// <summary>A place of the library (above the pane's separator), as opposed to Search and Sources.</summary>
    public static bool IsLibraryTag(string tag) => tag is not ("search" or "sources");

    /// <summary>
    /// The face for a typeface: Segoe UI Variable, then Cascadia Mono for mono and Georgia for serif
    /// (ADR-0020 §2; no bundled fonts). Windows has no rounded face: Rounded keeps the system's.
    /// </summary>
    public static string FontFamily(Typeface typeface) => typeface switch
    {
        Typeface.Serif => "Georgia",
        Typeface.Mono => "Cascadia Mono, Consolas",
        _ => "Segoe UI Variable Text, Segoe UI",
    };

    /// <summary>The aurora field's opacity: its glows are drawn at the default strength, scaled down (never up) to the slice's.</summary>
    public static double AuroraOpacity(LookState look) => Math.Clamp(look.Aurora / LookState.DefaultAurora, 0, 1);
}
