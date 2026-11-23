using System.Collections.Specialized;
using System.ComponentModel;
using Jellyfesh.App.Controls;
using Jellyfesh.App.Platform;
using Jellyfesh.App.Shell;
using Jellyfesh.App.Theming;
using Jellyfesh.App.Views;
using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Jellyfesh.Interop;
using Jellyfesh.Services;
using Jellyfesh.ViewModels;
using Microsoft.UI;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Navigation;
using Windows.UI.ViewManagement;
using WinRT.Interop;
using VirtualKey = Windows.System.VirtualKey;

namespace Jellyfesh.App;

/// <summary>
/// The main window: wires the view model to the window's platform parts (media controls, taskbar,
/// icon, device notifications, backdrop) and keeps the drawing-time views active only while they
/// can be seen, so a minimised or covered-by-Now-Playing view draws nothing (Still mode).
/// </summary>
public sealed partial class MainWindow : Window
{
    /// <summary>Shift+Right and Shift+Left skip this far (docs/ui.md, "Keyboard").</summary>
    private const double SkipSeconds = 5;

    /// <summary>The page of each place: the sidebar's, the phone tabs' (Library opens the library place last shown) and Settings.</summary>
    private static readonly Dictionary<string, Type> TopPages = new()
    {
        ["home"] = typeof(HomePage),
        ["albums"] = typeof(AlbumsPage),
        ["artists"] = typeof(ArtistsPage),
        ["songs"] = typeof(SongsPage),
        ["playlists"] = typeof(PlaylistsPage),
        ["favorites"] = typeof(FavoritesPage),
        ["search"] = typeof(SearchPage),
        ["sources"] = typeof(SourcesPage),
        ["settings"] = typeof(SettingsPage),
    };

    /// <summary>Each place's name (a string key) and glyph (a resource of Styles/Controls.xaml).</summary>
    private static readonly Dictionary<string, (string Label, string Glyph)> PlaceLooks = new()
    {
        ["home"] = ("Place_Home", "IconHome"),
        ["albums"] = ("Place_Albums", "IconAlbum"),
        ["artists"] = ("Place_Artists", "IconArtist"),
        ["songs"] = ("Place_Songs", "IconNote"),
        ["playlists"] = ("Place_Playlists", "IconPlaylist"),
        ["favorites"] = ("Place_Favourites", "IconHeart"),
        ["search"] = ("Place_Search", "IconSearch"),
        ["sources"] = ("Place_Sources", "IconSources"),
        ["library"] = ("Place_Library", "IconLibrary"),
        ["settings"] = ("Place_Settings", "IconSettings"),
    };

    /// <summary>Detail pages select their section's place.</summary>
    private static readonly Dictionary<Type, string> Sections = new()
    {
        [typeof(AlbumPage)] = "albums",
        [typeof(ArtistPage)] = "artists",
        [typeof(PlaylistPage)] = "playlists",
        [typeof(SourceBrowsePage)] = "sources",
        [typeof(SourceDiscoverPage)] = "sources",
    };

    private readonly AppViewModel _vm;
    private readonly ShellPlatform _platform;
    private readonly BackdropController _backdrop;
    private readonly UISettings _ui = new();
    private readonly AccessibilitySettings _accessibility = new();
    private readonly DispatcherQueueTimer _noticeTimer;
    private readonly nint _hwnd;
    private WindowShell? _shell;
    private MediaTransport? _media;
    private AudioDeviceNotifications? _devices;
    private bool _visible = true;
    private bool _nowPlayingOpen;
    private ShellLayout _layout = ShellLayout.Initial;
    private bool _layoutApplied;

    private bool _compact;
    private Windows.Graphics.SizeInt32? _fullSize;
    private bool _themeApplied;
    private Notification? _shownNotice;
    private readonly HashSet<string> _failedParts = [];
    private DesktopLyricsWindow? _desktopLyrics;

    /// <summary>The sidebar's places, the phone tabs and the Library tab's switcher, by place (made from the look's order).</summary>
    private readonly Dictionary<string, SidebarItem> _places = [];
    private readonly Dictionary<string, SidebarItem> _tabs = [];
    private readonly Dictionary<string, SidebarItem> _libraryTabs = [];
    private readonly SidebarItem _settingsItem;

    /// <summary>The place of the page shown, and the library place last shown (where the Library tab goes).</summary>
    private string? _section;
    private string _lastLibraryPlace = "albums";

    /// <summary>Desktop Lyrics while open (the More menu unlocks it).</summary>
    internal DesktopLyricsWindow? DesktopLyrics => _desktopLyrics;

    public MainWindow(ICoreClient core)
    {
        // The view model exists before the XAML, so the controls it creates can bind to it.
        var queue = DispatcherQueue.GetForCurrentThread();
        var l = new ResourceLocalizer();
        _platform = new ShellPlatform(this, l);
        IProStore? store = null;
#if JF_DISTRIBUTION_MICROSOFT_STORE
        // The Store build sells Pro through the Store and reports its licence to the core.
        store = Optional("the Microsoft Store", () => StorePurchases.Create(WindowNative.GetWindowHandle(this), core));
#endif
        _vm = new AppViewModel(
            core,
            new WinUiDispatcher(queue),
            l,
            _platform,
            PortDrawingTime.Instance,
            CoreClock.Calibrate(core.MonotonicNowNs),
            new DispatcherOneShotTimer(queue),
            store,
            AppHost.Website);
        AppHost.Set(_vm, _platform, this);

        InitializeComponent();

        // Open AI Lyrics Settings (the Lyrics pane): Settings › Extensions.
        _vm.AiLyrics.SettingsRequested += () =>
        {
            CloseNowPlaying();
            Navigate(typeof(SettingsPage), SettingsSection.Extensions);
        };

        // The Pro sheet and Enter Licence Key, from any Pro action (docs/ui.md).
        _vm.Pro.SheetRequested += () => _ = ProDialogs.ShowSheetAsync(_platform, _vm.Pro, l);
        _vm.Pro.LicenceKeyRequested += key => _ = ProDialogs.ShowLicenceKeyAsync(_platform, key, l);

        // The keyboard's Back key. In code because WinUI's XAML parser has no name for it: its table
        // of VirtualKey values (EnumValueTable) stops at the original keys, so Key="GoBack" fails to load.
        var backKey = new KeyboardAccelerator { Key = VirtualKey.GoBack };
        backKey.Invoked += OnBackKey;
        Root.KeyboardAccelerators.Add(backKey);

        // Ctrl+, opens Settings (docs/ui.md, "Keyboard"); VirtualKey has no name for the comma key (VK_OEM_COMMA).
        var settingsKey = new KeyboardAccelerator { Key = (VirtualKey)0xBC, Modifiers = Windows.System.VirtualKeyModifiers.Control };
        settingsKey.Invoked += OnSettingsKey;
        Root.KeyboardAccelerators.Add(settingsKey);

        // The places: the sidebar's and the phone tabs' from the look, Settings always at the sidebar's foot.
        _settingsItem = MakePlace("settings", "JfSidebarItemStyle", "Places");
        Foot.Children.Add(_settingsItem);
        Optional("the places", BuildPlaces);

        // Shadows of the library (covers, groups, the player bar) fall on the field (ThemeShadow receivers).
        Optional("the library's shadows", () => Elevation.AddReceiver(Elevation.Library, Field));

        Title = "Jellyfesh";
        Optional("the title bar", () =>
        {
            ExtendsContentIntoTitleBar = true;
            SetTitleBar(AppTitleBar);
        });
        _hwnd = WindowNative.GetWindowHandle(this);
        Optional("placing the window", PlaceWindow);
        ContentFrame.NavigationFailed += OnNavigationFailed;

        _backdrop = new BackdropController(this);
        _backdrop.Changed += ApplyMaterial;
        _noticeTimer = queue.CreateTimer();
        _noticeTimer.IsRepeating = false;
        _noticeTimer.Tick += (_, _) =>
        {
            _noticeTimer.Stop();
            Notice.IsOpen = false;
        };

        _vm.ThemeChanged += ApplyTheme;
        _vm.PropertyChanged += OnAppPropertyChanged;
        _vm.Player.PropertyChanged += OnPlayerPropertyChanged;
        _vm.Notifications.Items.CollectionChanged += OnNotificationsChanged;
        NowPlaying.CloseRequested += (_, _) => CloseNowPlaying();
        NowPlaying.CompactRequested += (_, _) => EnterCompact();
        Compact.ExpandRequested += (_, _) => ExitCompact();
        NowPlaying.ViewsChanged += (_, _) => UpdateActivity();
        Mini.OpenRequested += (_, _) => OpenNowPlaying();
        Bar.OpenRequested += (_, panel) => OpenNowPlaying(panel);
        Bar.MiniPlayerRequested += (_, _) => EnterCompact();

        _ui.ColorValuesChanged += (_, _) => queue.TryEnqueue(ReadSystemPreferences);
        // Either may be refused in a desktop app (both are logged): ColorValuesChanged above still
        // follows high contrast, whose colours change with it, and the next one reads the animation setting.
        TryHook("live changes of the animation setting", () => _ui.AnimationsEnabledChanged += (_, _) => queue.TryEnqueue(ReadSystemPreferences));
        TryHook("live changes of high contrast", () => _accessibility.HighContrastChanged += (_, _) => queue.TryEnqueue(ApplyTheme));
        VisibilityChanged += OnVisibilityChanged;
        Closed += OnClosed;
        Root.AddHandler(UIElement.PointerPressedEvent, new PointerEventHandler(OnRootPointerPressed), handledEventsToo: true);

        _vm.Start();
        Optional("the system's colours and animation setting", ReadSystemPreferences);
        _backdrop.Refresh();

        // The Windows parts: each that fails is logged and left out, and the window opens without it.
        _shell = Optional("the taskbar buttons and the window icon", () => new WindowShell(_hwnd, _vm.Player, l));
        Optional("the window icon", () => _shell?.SetIcon(_vm.AppIcon));
        _media = Optional("the system media controls", () => new MediaTransport(_hwnd, _vm.Player, _vm.Services, queue));
        _devices = Optional("the audio device notifications", () => new AudioDeviceNotifications(queue, c => _vm.Services.Send(c)));
        _vm.Services.Send(new CoreCommand.RefreshDevices());

        SelectSection("home");
        UpdateActivity();
    }

    public AppViewModel ViewModel => _vm;

    /// <summary>Shows the window after another start was redirected here (single instance).</summary>
    public void BringToFront()
    {
        if (Win32.IsIconic(_hwnd))
        {
            Win32.ShowWindow(_hwnd, Win32.SW_RESTORE);
        }

        AppWindow.Show();
        Activate();
        Win32.SetForegroundWindow(_hwnd);
    }

    /// <summary>Opens a page in the content frame (detail pages pass what to show).</summary>
    public void Navigate(Type page, object? parameter = null)
    {
        if (_nowPlayingOpen)
        {
            CloseNowPlaying();
        }

        if (ContentFrame.CurrentSourcePageType == page && parameter is null)
        {
            return;
        }

        // Pages fade in and rise 8 px (OnNavigated, Misty's motion tokens), not the system's transitions.
        ContentFrame.Navigate(page, parameter, new SuppressNavigationTransitionInfo());
    }

    /// <summary>Opens the search page with its box focused (Ctrl+E).</summary>
    public void ShowSearch()
    {
        ExitCompact();
        SelectSection("search");
        if (ContentFrame.Content is SearchPage search)
        {
            search.FocusBox();
        }
    }

    /// <summary>Settings (Ctrl+,): the page in the main window (ADR-0023).</summary>
    public void ShowSettings()
    {
        ExitCompact();
        SelectSection("settings");
    }

    public void OpenNowPlaying(NowPlayingPanel panel = NowPlayingPanel.Default)
    {
        ExitCompact();
        if (!_vm.Player.HasTrack)
        {
            return;
        }

        if (_nowPlayingOpen)
        {
            NowPlaying.ShowPanel(panel);
            return;
        }

        _nowPlayingOpen = true;
        if (!_vm.ReduceMotion)
        {
            // The artwork grows from the mini player's or the bar's (a connected animation).
            NowPlaying.PrepareArtworkFrom(ShowsBar ? Bar.ArtElement : Mini.ArtElement);
        }

        // Under reduced motion Now Playing fades in (dur-1) without moving.
        NowPlaying.Open(animate: true, panel);
        UpdateActivity();
    }

    /// <summary>
    /// The compact mini player: the window becomes a small always-on-top player (Windows' compact
    /// overlay) until the person asks for the full window again (Ctrl+M or its button).
    /// </summary>
    public void EnterCompact()
    {
        if (_compact || !_vm.Player.HasTrack)
        {
            return;
        }

        _compact = true;
        _fullSize = AppWindow.Size;
        if (_nowPlayingOpen)
        {
            _nowPlayingOpen = false;
            NowPlaying.Close(animate: false);
        }

        AppTitleBar.Visibility = Visibility.Collapsed;
        ShellGrid.Visibility = Visibility.Collapsed;
        Notice.IsOpen = false;
        Compact.Visibility = Visibility.Visible;
        SetTitleBar(Compact.DragRegion);
        AppWindow.SetPresenter(AppWindowPresenterKind.CompactOverlay);
        var side = (int)(340 * Math.Max(1.0, Win32.GetDpiForWindow(_hwnd) / 96.0));
        AppWindow.Resize(new Windows.Graphics.SizeInt32(side, side));
        UpdateActivity();
    }

    public void ExitCompact()
    {
        if (!_compact)
        {
            return;
        }

        _compact = false;
        AppWindow.SetPresenter(AppWindowPresenterKind.Default);
        if (_fullSize is { } size)
        {
            AppWindow.Resize(size);
        }

        Compact.Visibility = Visibility.Collapsed;
        AppTitleBar.Visibility = Visibility.Visible;
        ShellGrid.Visibility = Visibility.Visible;
        SetTitleBar(AppTitleBar);
        UpdateActivity();
    }

    public void CloseNowPlaying()
    {
        if (!_nowPlayingOpen)
        {
            return;
        }

        _nowPlayingOpen = false;
        NowPlaying.Close(animate: true);
        UpdateActivity();
        if (ShowsBar)
        {
            Bar.Focus(FocusState.Programmatic);
        }
        else
        {
            Mini.Focus(FocusState.Programmatic);
        }
    }

    /// <summary>The window's width picks the layout: the sidebar or the phone tabs, the player bar or the capsule.</summary>
    private void OnRootSizeChanged(object sender, SizeChangedEventArgs e)
    {
        if (ShellLayout.For(e.NewSize.Width, _vm.Player.HasTrack, _vm.Look.MiniPlayer) != _layout)
        {
            UpdateActivity();
        }
    }

    /// <summary>A first size of about 1280 × 860 epx, kept inside the screen, centred.</summary>
    private void PlaceWindow()
    {
        var scale = Math.Max(1.0, Win32.GetDpiForWindow(_hwnd) / 96.0);
        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Primary).WorkArea;
        var width = (int)Math.Min(1280 * scale, area.Width * 0.92);
        var height = (int)Math.Min(860 * scale, area.Height * 0.92);
        AppWindow.MoveAndResize(new Windows.Graphics.RectInt32(
            area.X + ((area.Width - width) / 2), area.Y + ((area.Height - height) / 2), width, height));
    }

    /// <summary>
    /// Subscribes to a system notification the window can do without: one Windows refuses (an older
    /// version, ADR-0007's minimum is 1809, or a notification a desktop app may not have) is logged
    /// with its HRESULT, and only live updates are lost.
    /// </summary>
    private static void TryHook(string what, Action hook)
    {
        try
        {
            hook();
        }
        catch (Exception e)
        {
            AppLog.Write($"{what}: not available on this PC", e);
        }
    }

    /// <summary>
    /// Runs a part of the window that it can do without (the title bar, icons, media controls, …):
    /// one that fails on this PC is logged, once per part, and the window goes on without it.
    /// </summary>
    private void Optional(string what, Action action)
    {
        try
        {
            action();
        }
        catch (Exception e)
        {
            if (_failedParts.Add(what))
            {
                AppLog.Write($"{what}: not available on this PC", e);
            }
        }
    }

    private T? Optional<T>(string what, Func<T> create)
        where T : class
    {
        T? made = null;
        // A statement lambda, so this calls Optional(string, Action): the expression
        // lambda `() => made = create()` is a Func<T> and called this method again,
        // forever (the stack overflow CI's smoke test found).
        Optional(what, () => { made = create(); });
        return made;
    }

    private void OnNavigationFailed(object sender, NavigationFailedEventArgs e)
    {
        AppLog.Write($"opening {e.SourcePageType?.Name ?? "a page"} failed", e.Exception);
        if (Startup.SmokeTest)
        {
            e.Handled = true;
            Startup.Fail(e.Exception, xamlWorks: true);
        }
    }

    // ---- the smoke test (Jellyfesh.exe --smoke-test, Shell/SmokeTest.cs) ----

    /// <summary>Every place, in the sidebar's order, then Settings: what the smoke test opens.</summary>
    internal static IReadOnlyList<string> SmokeSections { get; } = [.. TopPages.Keys];

    /// <summary>The places the smoke test opens again in the phone layout (the tabs and the Library switcher).</summary>
    internal static IReadOnlyList<string> SmokePhoneSections { get; } = ["home", "albums", "search", "settings"];

    /// <summary>The window's content, whose first frames the smoke test waits for.</summary>
    internal FrameworkElement ContentRoot => Root;

    /// <summary>Opens a place as the sidebar would, and returns the page that now shows.</summary>
    internal FrameworkElement? ShowSection(string tag)
    {
        SelectSection(tag);
        return ContentFrame.Content as FrameworkElement;
    }

    /// <summary>The smoke test's phone layout: the window this many effective pixels wide.</summary>
    internal void ResizeForSmokeTest(double width, double height)
    {
        var scale = Math.Max(1.0, Win32.GetDpiForWindow(_hwnd) / 96.0);
        AppWindow.Resize(new Windows.Graphics.SizeInt32((int)(width * scale), (int)(height * scale)));
    }

    /// <summary>Whether the window shows the phone layout (the smoke test checks it got there).</summary>
    internal bool UsesPhoneLayout => !_layout.Sidebar;

    // ---- what draws while it can be seen (Still mode) ----

    /// <summary>The player bar rather than the capsule: windows 900 epx and wider with the look's Bar style (ADR-0020).</summary>
    private bool ShowsBar => _layout.Bar;

    /// <summary>
    /// The seek bar and lyrics draw only while Now Playing is open, the mini player's ring only while
    /// it shows, and nothing while the window is minimised or hidden; the ticker then sleeps.
    /// </summary>
    private void UpdateActivity()
    {
        var player = _vm.Player;
        ApplyLayout();
        var floating = player.HasTrack && !_nowPlayingOpen && !_compact;
        var mini = floating && !ShowsBar;
        var bar = floating && ShowsBar;
        Mini.Visibility = mini ? Visibility.Visible : Visibility.Collapsed;
        Bar.Visibility = bar ? Visibility.Visible : Visibility.Collapsed;
        ContentFade.Visibility = _layout.FadeHeight > 0 ? Visibility.Visible : Visibility.Collapsed;
        player.Progress.IsActive = _visible && _nowPlayingOpen && !_compact;
        player.BarProgress.IsActive = _visible && bar;
        player.MiniProgress.IsActive = _visible && (mini || _compact);
        if (_compact)
        {
            Compact.UpdateSteps();
        }
        else
        {
            Mini.UpdateSteps();
        }
        _vm.Lyrics.IsActive = _visible && _nowPlayingOpen && !_compact && NowPlaying.ShowsLyrics;
        _vm.Services.Ticker.Invalidate();
    }

    private void OnVisibilityChanged(object sender, WindowVisibilityChangedEventArgs args)
    {
        _visible = args.Visible;
        UpdateActivity();
    }

    private void OnPlayerPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(PlayerViewModel.HasTrack))
        {
            if (!_vm.Player.HasTrack)
            {
                CloseNowPlaying();
                ExitCompact();
            }

            UpdateActivity();
        }
    }

    private void OnAppPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(AppViewModel.IsScanning) or nameof(AppViewModel.ScanText))
        {
            ScanPanel.Visibility = _vm.IsScanning ? Visibility.Visible : Visibility.Collapsed;
            ScanText.Text = _vm.ScanText;
        }
        else if (e.PropertyName == nameof(AppViewModel.Settings))
        {
            SyncDesktopLyrics();
        }
        else if (e.PropertyName == nameof(AppViewModel.Look))
        {
            Optional("the look", ApplyLook);
        }
    }

    /// <summary>
    /// The Look slice, live (ADR-0020 §2): the sidebar's places in its order (a separator before
    /// Search and Sources), the phone tabs and the Library switcher in the same order, the platform's
    /// face for its typeface, the aurora's strength, the glass (the Transparency setting times the
    /// look's glass) and Desktop Lyrics' highlight. The lyrics, the lyrics bar, Now Playing's layout
    /// and the bottom player's style follow it in their views.
    /// </summary>
    private void ApplyLook()
    {
        var look = _vm.Look;
        BuildPlaces();
        var face = new Microsoft.UI.Xaml.Media.FontFamily(LookRules.FontFamily(look.Typeface));
        ContentFrame.FontFamily = face;
        OpenFilesButton.FontFamily = face;
        foreach (var item in _places.Values.Concat(_tabs.Values).Concat(_libraryTabs.Values).Append(_settingsItem))
        {
            item.FontFamily = face;
        }

        NowPlaying.FontFamily = face;
        Bar.FontFamily = face;
        Mini.FontFamily = face;
        Field.Opacity = LookRules.AuroraOpacity(look);
        UpdateActivity(); // the mini-player style: the bar, a compact bar or the floating capsule
        ThemeBrushes.SetTransparency(LookRules.GlassTransparency(look, _vm.Settings.Transparency));
        if (_desktopLyrics is { } desktop)
        {
            desktop.Highlight = LyricsLook.BarHighlight(look);
        }
    }

    /// <summary>Desktop Lyrics follows its setting: opened when on, closed when off; its close button turns it off.</summary>
    private void SyncDesktopLyrics()
    {
        var wanted = _vm.Settings.DesktopLyrics;
        if (wanted && _desktopLyrics is null)
        {
            _desktopLyrics = Optional("Desktop Lyrics", () =>
            {
                var window = new DesktopLyricsWindow(_vm.LyricsBar, _vm.Services.L, () =>
                    _vm.Services.Send(new CoreCommand.SetSettings(_vm.Services.Hub.State.Settings with { DesktopLyrics = false })));
                window.Closed += (_, _) => _desktopLyrics = null;
                window.Highlight = LyricsLook.BarHighlight(_vm.Look);
                window.Activate();
                return window;
            });
        }
        else if (!wanted && _desktopLyrics is { } open)
        {
            _desktopLyrics = null;
            Optional("closing Desktop Lyrics", open.Close);
        }
    }

    // ---- appearance ----

    private void ReadSystemPreferences()
    {
        // A dark system background means the dark app mode.
        var background = _ui.GetColorValue(UIColorType.Background);
        var dark = (background.R + background.G + background.B) / 3 < 128;
        _vm.SetSystemPreferences(dark, !_ui.AnimationsEnabled);
    }

    /// <summary>Light or dark, the theme colours (crossfaded unless motion is reduced), the material and the icons.</summary>
    private void ApplyTheme()
    {
        var dark = _vm.Appearance == Core.Appearance.Dark;
        Root.RequestedTheme = dark ? ElementTheme.Dark : ElementTheme.Light;
        if (_accessibility.HighContrast)
        {
            ThemeBrushes.ApplyHighContrast(_ui);
        }
        else
        {
            ThemeBrushes.Apply(_vm.Palette, instant: !_themeApplied, quick: _vm.ReduceMotion);
        }

        _themeApplied = true;
        ThemeBrushes.SetTransparency(LookRules.GlassTransparency(_vm.Look, _vm.Settings.Transparency));
        _backdrop.SetReduceEffects(_vm.Settings.ReduceEffects);
        Optional("the window icon", () => _shell?.SetIcon(_vm.AppIcon));
        Optional("the title bar's icon", () => TitleIcon.ImageSource = new BitmapImage(AppAssets.IconPng(_vm.AppIcon, 32)));
        Optional("the sidebar's icon", () => BrandIcon.Source = new BitmapImage(AppAssets.IconPng(_vm.AppIcon, 64)));
        Optional("the caption buttons' colours", () => ApplyCaptionButtons(dark));
        NowPlaying.ApplyTheme();
    }

    private void ApplyMaterial()
    {
        // Mica shows through a transparent root; without it the window shows the field's high sky.
        Root.Background = _backdrop.HasMica ? null : (Brush)Application.Current.Resources["JfSkyHigh"];
    }

    private void ApplyCaptionButtons(bool dark)
    {
        var bar = AppWindow.TitleBar;
        var ink = dark ? Colors.White : Colors.Black;
        bar.ButtonBackgroundColor = Colors.Transparent;
        bar.ButtonInactiveBackgroundColor = Colors.Transparent;
        bar.ButtonForegroundColor = ink;
        bar.ButtonHoverForegroundColor = ink;
        bar.ButtonPressedForegroundColor = ink;
        bar.ButtonHoverBackgroundColor = Windows.UI.Color.FromArgb(24, ink.R, ink.G, ink.B);
        bar.ButtonPressedBackgroundColor = Windows.UI.Color.FromArgb(40, ink.R, ink.G, ink.B);
        bar.ButtonInactiveForegroundColor = Windows.UI.Color.FromArgb(128, ink.R, ink.G, ink.B);
    }

    // ---- navigation: the sidebar, the phone tabs and the Library switcher (ADR-0023) ----

    /// <summary>
    /// Makes the places from the look's order (ADR-0020 §2): the sidebar's, with a separator before
    /// Search and Sources; the phone tabs (the library's places as one Library tab, Settings last);
    /// and the Library tab's switcher. Then marks the place shown.
    /// </summary>
    private void BuildPlaces()
    {
        var look = _vm.Look;
        Places.Children.Clear();
        _places.Clear();
        var library = true;
        foreach (var tag in LookRules.SidebarTags(look))
        {
            if (!PlaceLooks.ContainsKey(tag))
            {
                continue;
            }

            if (library && !LookRules.IsLibraryTag(tag) && Places.Children.Count > 0)
            {
                // The web's separator: a hairline 8 epx above and below, 12 in from the sides.
                Places.Children.Add(new Border
                {
                    Height = 1,
                    Margin = new Thickness(12, 8, 12, 8),
                    Background = (Brush)Application.Current.Resources["JfLineSoft"],
                });
            }

            library = LookRules.IsLibraryTag(tag);
            var item = MakePlace(tag, "JfSidebarItemStyle", "Places");
            Places.Children.Add(item);
            _places[tag] = item;
        }

        TabItems.Children.Clear();
        TabItems.ColumnDefinitions.Clear();
        _tabs.Clear();
        foreach (var tag in LookRules.PhoneTabs(look))
        {
            var item = MakePlace(tag, "JfPhoneTabStyle", "Tabs");
            TabItems.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            Grid.SetColumn(item, TabItems.Children.Count);
            TabItems.Children.Add(item);
            _tabs[tag] = item;
        }

        LibraryTabs.Children.Clear();
        _libraryTabs.Clear();
        foreach (var tag in LookRules.LibraryTags(look))
        {
            var item = MakePlace(tag, "JfLibraryTabStyle", "LibraryTabs");
            LibraryTabs.Children.Add(item);
            _libraryTabs[tag] = item;
        }

        MarkPlace(_section);
    }

    /// <summary>A place in one of the three looks; choosing it opens its page.</summary>
    private SidebarItem MakePlace(string tag, string style, string group)
    {
        var (label, glyph) = PlaceLooks[tag];
        var item = new SidebarItem
        {
            Place = tag,
            Label = _vm.Services.L.Get(label),
            Glyph = (string)Application.Current.Resources[glyph],
            GroupName = group,
            Style = (Style)Application.Current.Resources[style],
        };
        item.Click += OnPlaceClick;
        return item;
    }

    /// <summary>Choosing a place opens its page, also the place already chosen (from an album back to Albums).</summary>
    private void OnPlaceClick(object sender, RoutedEventArgs e)
    {
        if (sender is SidebarItem item)
        {
            SelectSection(item.Place);
        }
    }

    /// <summary>Open Files… plays files without adding them to the library.</summary>
    private void OnOpenFilesClick(object sender, RoutedEventArgs e) => _vm.OpenFilesCommand.Execute(null);

    private void SelectSection(string tag)
    {
        if (tag == "library")
        {
            tag = _libraryTabs.ContainsKey(_lastLibraryPlace) ? _lastLibraryPlace : LookRules.LibraryTags(_vm.Look).FirstOrDefault() ?? "albums";
        }

        if (TopPages.TryGetValue(tag, out var page))
        {
            Navigate(page);
        }

        // Navigate does nothing for the page already shown; the place stays marked either way.
        MarkPlace(SectionOf(ContentFrame.CurrentSourcePageType));
    }

    /// <summary>The place a page belongs to: its own, or for a detail page its section's.</summary>
    private static string? SectionOf(Type? page) =>
        page is null ? null : TopPages.FirstOrDefault(p => p.Value == page).Key ?? Sections.GetValueOrDefault(page);

    /// <summary>Marks the place shown in the sidebar, the phone tabs and the Library switcher.</summary>
    private void MarkPlace(string? tag)
    {
        _section = tag;
        if (tag is not null && LookRules.PhoneTab(tag) == "library")
        {
            _lastLibraryPlace = tag;
        }

        foreach (var (place, item) in _places)
        {
            item.IsChecked = place == tag;
        }

        _settingsItem.IsChecked = tag == "settings";
        var tab = tag is null ? null : LookRules.PhoneTab(tag);
        foreach (var (place, item) in _tabs)
        {
            item.IsChecked = place == tab;
        }

        foreach (var (place, item) in _libraryTabs)
        {
            item.IsChecked = place == tag;
        }

        UpdateLibrarySwitcher();
    }

    /// <summary>The Library tab's switcher shows above the library's pages in the phone layout.</summary>
    private void UpdateLibrarySwitcher() =>
        LibrarySwitcher.Visibility = !_layout.Sidebar && _section is { } tag && _libraryTabs.ContainsKey(tag)
            ? Visibility.Visible
            : Visibility.Collapsed;

    /// <summary>
    /// The layout for the window's width (ShellLayout, Jellyfesh Web's measures): the sidebar or the
    /// phone tabs, where the bottom player floats, the fade above it, and the pages' measures. The
    /// title bar names Jellyfesh only in the phone layout; the sidebar's wordmark does otherwise.
    /// </summary>
    private void ApplyLayout()
    {
        var width = Root.ActualWidth > 0 ? Root.ActualWidth : 1280;
        var layout = ShellLayout.For(width, _vm.Player.HasTrack, _vm.Look.MiniPlayer);
        var sidebarChanged = layout.Sidebar != _layout.Sidebar || !_layoutApplied;
        _layout = layout;
        _layoutApplied = true;
        if (sidebarChanged)
        {
            SidebarColumn.Width = new GridLength(layout.Sidebar ? ShellLayout.SidebarColumn : 0);
            Sidebar.Visibility = layout.Sidebar ? Visibility.Visible : Visibility.Collapsed;
            Tabs.Visibility = layout.Sidebar ? Visibility.Collapsed : Visibility.Visible;
            Optional("the title bar's name", () =>
            {
                AppTitleBar.Title = layout.Sidebar ? string.Empty : "Jellyfesh";
                AppTitleBar.IconSource = layout.Sidebar ? null : TitleIcon;
            });
        }

        Dock.Margin = new Thickness(layout.DockSide, 0, layout.DockSide, layout.DockBottom);
        Mini.Place(_vm.Look.MiniPlayer, width - (layout.Sidebar ? ShellLayout.SidebarColumn : 0) - (2 * layout.DockSide));
        ContentFade.Height = Math.Max(1, layout.FadeHeight);
        Bar.ShowsVolumeLine = layout.VolumeLine;
        UpdateLibrarySwitcher();
        AppHost.Metrics.Set(layout);
    }

    private void OnNavigated(object sender, NavigationEventArgs e)
    {
        // Pages fade in and rise 8 px (dur-2 Fluid); going back fades (dur-1), and so does every page
        // under reduced motion.
        if (e.Content is UIElement page)
        {
            var fadeOnly = e.NavigationMode == NavigationMode.Back || _vm.ReduceMotion;
            var duration = TokenValues.Duration(fadeOnly ? 1 : 2);
            Motion.Enter(page, new System.Numerics.Vector3(0, fadeOnly ? 0 : 8, 0), duration, TokenValues.Fluid);
        }

        AppTitleBar.IsBackButtonEnabled = ContentFrame.CanGoBack;
        MarkPlace(SectionOf(e.SourcePageType));
    }

    private void GoBack()
    {
        if (_compact)
        {
            ExitCompact();
        }
        else if (_nowPlayingOpen)
        {
            CloseNowPlaying();
        }
        else if (ContentFrame.CanGoBack)
        {
            ContentFrame.GoBack();
        }
    }

    private void OnBackRequested(TitleBar sender, object args) => GoBack();

    private void OnRootPointerPressed(object sender, PointerRoutedEventArgs e)
    {
        // The mouse's back button.
        if (e.GetCurrentPoint(Root).Properties.IsXButton1Pressed)
        {
            GoBack();
            e.Handled = true;
        }
    }

    // ---- keys (also listed in docs/windows.md) ----

    /// <summary>Text boxes keep their keys: Space types, Esc and the arrows edit (docs/ui.md, "Keyboard").</summary>
    private static bool IsTextInput(object? element) => element is TextBox or PasswordBox or RichEditBox or AutoSuggestBox;

    /// <summary>Whether a text box has focus, in the window or in a dialog over it.</summary>
    private bool IsTyping() => Root.XamlRoot is { } root && IsTextInput(FocusManager.GetFocusedElement(root));

    private static bool IsDown(VirtualKey key) =>
        Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(key).HasFlag(Windows.UI.Core.CoreVirtualKeyStates.Down);

    /// <summary>
    /// Ctrl+Right and Ctrl+Left (next and previous song), Ctrl+Up and Ctrl+Down (volume), Shift+Right
    /// and Shift+Left (skip 5 seconds). They are read before the focused control sees them, so a
    /// list or slider with focus does not take them, except a text box, which keeps its own.
    /// </summary>
    private void OnRootPreviewKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Handled || e.Key is not (VirtualKey.Left or VirtualKey.Right or VirtualKey.Up or VirtualKey.Down) || IsTextInput(e.OriginalSource))
        {
            return;
        }

        var ctrl = IsDown(VirtualKey.Control);
        var shift = IsDown(VirtualKey.Shift);
        if (ctrl == shift || IsDown(VirtualKey.Menu))
        {
            return; // exactly one of Ctrl and Shift
        }

        var player = _vm.Player;
        Action? action = (e.Key, ctrl) switch
        {
            (VirtualKey.Right, true) when player.HasTrack => () => player.NextCommand.Execute(null),
            (VirtualKey.Left, true) when player.HasTrack => () => player.PreviousCommand.Execute(null),
            (VirtualKey.Up, true) => () => player.VolumeUpCommand.Execute(null),
            (VirtualKey.Down, true) => () => player.VolumeDownCommand.Execute(null),
            (VirtualKey.Right, false) when player.HasTrack => () => player.SeekBy(SkipSeconds),
            (VirtualKey.Left, false) when player.HasTrack => () => player.SeekBy(-SkipSeconds),
            _ => null,
        };
        if (action is not null)
        {
            action();
            e.Handled = true;
        }
    }

    /// <summary>Space plays or pauses, unless a text box has focus (a focused button or list takes Space first).</summary>
    private void OnSpaceKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (!IsTyping())
        {
            Run(args, _vm.Player.PlayPauseCommand.Execute);
        }
    }

    /// <summary>Esc goes back (closing Now Playing first), unless a text box has focus.</summary>
    private void OnEscapeKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (!IsTyping())
        {
            Run(args, _ => GoBack());
        }
    }

    private void OnPlayPauseKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Run(args, _vm.Player.PlayPauseCommand.Execute);

    private void OnPreviousKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Run(args, _vm.Player.PreviousCommand.Execute);

    private void OnShuffleKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Run(args, _vm.Player.ToggleShuffleCommand.Execute);

    private void OnRepeatKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Run(args, _vm.Player.CycleRepeatCommand.Execute);

    private void OnMuteKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Run(args, _vm.Player.ToggleMuteCommand.Execute);

    private void OnVolumeDownKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Run(args, _vm.Player.VolumeDownCommand.Execute);

    private void OnVolumeUpKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Run(args, _vm.Player.VolumeUpCommand.Execute);

    private void OnSearchKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Run(args, _ => ShowSearch());

    /// <summary>Ctrl+, opens Settings, unless a text box has focus.</summary>
    private void OnSettingsKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (!IsTyping())
        {
            Run(args, _ => ShowSettings());
        }
    }

    private void OnBackKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Run(args, _ => GoBack());

    private void OnNowPlayingKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Run(args, _ =>
    {
        if (_nowPlayingOpen)
        {
            CloseNowPlaying();
        }
        else
        {
            OpenNowPlaying();
        }
    });

    private void OnCompactKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Run(args, _ =>
    {
        if (_compact)
        {
            ExitCompact();
        }
        else
        {
            EnterCompact();
        }
    });

    private static void Run(KeyboardAcceleratorInvokedEventArgs args, Action<object?> action)
    {
        action(null);
        args.Handled = true;
    }

    // ---- notices ----

    private void OnNotificationsChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {
        var items = _vm.Notifications.Items;
        if (items.Count == 0)
        {
            Notice.IsOpen = false;
            _shownNotice = null;
            return;
        }

        var latest = items[^1];
        if (ReferenceEquals(latest, _shownNotice))
        {
            return;
        }

        _shownNotice = latest;
        Notice.Title = latest.Title;
        Notice.Message = latest.Message;
        Notice.Severity = latest.Severity switch
        {
            NotificationSeverity.Success => InfoBarSeverity.Success,
            NotificationSeverity.Warning => InfoBarSeverity.Warning,
            NotificationSeverity.Error => InfoBarSeverity.Error,
            _ => InfoBarSeverity.Informational,
        };
        Notice.IsOpen = true;

        // Good news fades on its own; problems stay until dismissed.
        _noticeTimer.Stop();
        if (latest.Severity is NotificationSeverity.Success or NotificationSeverity.Informational)
        {
            _noticeTimer.Interval = TimeSpan.FromSeconds(6);
            _noticeTimer.Start();
        }
    }

    private void OnNoticeClosed(InfoBar sender, InfoBarClosedEventArgs args)
    {
        if (_shownNotice is { } shown)
        {
            _shownNotice = null;
            _vm.Notifications.Dismiss(shown);
        }
    }

    private void OnClosed(object sender, WindowEventArgs args)
    {
        AppLog.Write("the window closed; shutting down");

        // A new start now opens a new window rather than handing itself to this process; and should
        // shutting down hang, the process still ends, so it never lingers without a window. The core
        // may wait up to a minute for plugin installs already under way.
        Program.ReleaseInstanceKey();
        if (Startup.WindowOpen)
        {
            Startup.ArmWatchdog(TimeSpan.FromMinutes(2), ExitCodes.ShutdownHung, "shutting down");
        }

        Optional("closing Desktop Lyrics", () => _desktopLyrics?.Close());
        Optional("closing the media controls", () => _media?.Dispose());
        Optional("closing the device notifications", () => _devices?.Dispose());
        Optional("closing the taskbar buttons", () => _shell?.Dispose());
        _vm.Dispose(); // saves the session and shuts the core down
        AppLog.Write("shut down");
    }
}
