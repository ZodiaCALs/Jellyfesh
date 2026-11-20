using CommunityToolkit.Mvvm.ComponentModel;
using Jellyfesh.Appearance;
using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Jellyfesh.Formatting;
using Jellyfesh.Services;

namespace Jellyfesh.ViewModels;

/// <summary>
/// The root of the shell's state: it starts the hub, keeps the player, queue and lyrics in step
/// with the core's slices, and makes page view models. One per window.
/// </summary>
public sealed partial class AppViewModel : ObservableObject, IDisposable
{
    private bool _systemIsDark;
    private bool _systemReducesMotion;
    private bool _lyricsHidden;
    private bool _disposed;

    [ObservableProperty]
    private AppSettings _settings = AppSettings.Default;

    [ObservableProperty]
    private Core.Appearance _appearance = Core.Appearance.Light;

    [ObservableProperty]
    private ThemePalette _palette = FallbackTheme.Value.Light;

    [ObservableProperty]
    private string? _backdropPath;

    [ObservableProperty]
    private string _appIcon = AppIcons.DefaultLight;

    [ObservableProperty]
    private bool _isScanning;

    [ObservableProperty]
    private string _scanText = string.Empty;

    [ObservableProperty]
    private bool _reduceMotion;

    public AppViewModel(
        ICoreClient core,
        IUiDispatcher ui,
        ILocalizer localizer,
        IShellPlatform platform,
        IDrawingTime time,
        IMonotonicClock clock,
        IOneShotTimer timer,
        IProStore? store = null,
        AboutInfo.Links? links = null)
    {
        var hub = new CoreHub(core, ui);
        var ticker = new PlaybackTicker(time.CreateClock(), clock, timer);
        Services = new ShellServices(hub, ui, localizer, platform, time, ticker);
        Player = new PlayerViewModel(Services);
        Queue = new QueueViewModel(Services);
        Lyrics = new LyricsPresenter(time);
        Pro = new ProViewModel(Services, store, links);
        Links = links;
        ticker.Attach(Lyrics);
    }

    public ShellServices Services { get; }

    public PlayerViewModel Player { get; }

    public QueueViewModel Queue { get; }

    public LyricsPresenter Lyrics { get; }

    /// <summary>Jellyfesh Pro: the Pro sheet and Settings › Extensions show it.</summary>
    public ProViewModel Pro { get; }

    /// <summary>The website's pages (About, the legal links), when the build knows the website.</summary>
    public AboutInfo.Links? Links { get; }

    public NotificationsViewModel Notifications => Services.Notifications;

    /// <summary>Raised after the theme, appearance or style changed, for the app to restyle (UI thread).</summary>
    public event Action? ThemeChanged;

    public void Start()
    {
        Services.Hub.Changed += OnChanged;
        Services.Hub.Notification += OnNotification;
        Services.Hub.Start();
    }

    /// <summary>The system's light/dark choice and its animation setting (UISettings in the app).</summary>
    public void SetSystemPreferences(bool isDark, bool reducesMotion)
    {
        _systemIsDark = isDark;
        _systemReducesMotion = reducesMotion;
        UpdateAppearance(Services.Hub.State);
    }

    public HomeViewModel CreateHome() => new(Services);

    public AlbumsViewModel CreateAlbums() => new(Services);

    public AlbumDetailViewModel CreateAlbum(Album album) => new(Services, album);

    public ArtistsViewModel CreateArtists() => new(Services);

    public ArtistDetailViewModel CreateArtist(Artist artist) => new(Services, artist);

    public SongsViewModel CreateSongs() => new(Services);

    public PlaylistsViewModel CreatePlaylists() => new(Services);

    public PlaylistDetailViewModel CreatePlaylist(PlaylistSummary playlist) => new(Services, playlist);

    public FavoritesViewModel CreateFavorites() => new(Services);

    public SearchViewModel CreateSearch() => new(Services);

    public SourcesViewModel CreateSources() => new(Services);

    /// <summary>A page of a source's catalogue: its top level, a folder, an artist, an album or a playlist.</summary>
    public SourceBrowseViewModel CreateSourceBrowse(SourceTarget target) => new(Services, target);

    public SettingsViewModel CreateSettings(Func<string, string?> iconPreview) => new(Services, Player, iconPreview, Pro);

    private void OnChanged(SliceChange changes)
    {
        var state = Services.Hub.State;
        Player.OnSlices(changes, state);
        if (changes.HasFlag(SliceChange.Pro))
        {
            Pro.Update(state.Pro);
        }

        if (changes.HasFlag(SliceChange.Queue))
        {
            Queue.OnQueue(state.Queue);
        }

        var remote = state.Remote.HasSession;
        if (changes.HasFlag(SliceChange.Lyrics) || changes.HasFlag(SliceChange.NowPlaying) || remote != _lyricsHidden)
        {
            // A remote session's service plays its own lyrics, if any: none are shown meanwhile.
            _lyricsHidden = remote;
            Lyrics.Load(remote ? null : state.Lyrics, remote ? null : state.NowPlaying?.Uid);
            Services.Ticker.Invalidate();
        }

        if (changes.HasFlag(SliceChange.Settings))
        {
            Settings = state.Settings;
            Lyrics.ShowTranslation = state.Settings.ShowTranslation;
            Lyrics.ShowRomanization = state.Settings.ShowRomanization;
        }

        if (changes.HasFlag(SliceChange.Scan))
        {
            IsScanning = state.Scanning;
            var p = Services.Hub.ScanProgress;
            ScanText = p is null ? string.Empty : Services.L.Format("Scan_Progress", p.FilesSeen, p.FilesChanged);
        }

        if (changes.HasFlag(SliceChange.Library))
        {
            Services.Thumbnails.Clear();
        }

        if (changes.HasFlag(SliceChange.Theme) || changes.HasFlag(SliceChange.Settings))
        {
            UpdateAppearance(state);
        }
    }

    private void OnNotification(CoreEvent e)
    {
        switch (e)
        {
            case CoreEvent.Buffering b:
                Player.OnBuffering(b);
                break;
            case CoreEvent.CommandFailed f when Services.IsHandled(f.Id):
                break; // the page that sent it shows it
            default:
                Notifications.Show(e);
                break;
        }
    }

    private void UpdateAppearance(CoreSnapshot state)
    {
        var appearance = AppearanceResolver.Effective(state.Settings.Appearance, _systemIsDark);
        var reduce = state.Settings.ReduceEffects || _systemReducesMotion;
        Lyrics.ReduceMotion = reduce;
        ReduceMotion = reduce;
        Appearance = appearance;
        Palette = AppearanceResolver.Palette(state.Theme, appearance);
        BackdropPath = state.Theme?.BackdropFor(appearance);
        AppIcon = AppIcons.Resolve(state.Settings.AppIcon, appearance);
        ThemeChanged?.Invoke();
    }

    /// <summary>Stops listening and shuts the core down (saving the session).</summary>
    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        Services.Hub.Changed -= OnChanged;
        Services.Hub.Notification -= OnNotification;
        Services.Hub.Dispose();
        Services.Core.Shutdown();
        Services.Core.Dispose();
    }
}
