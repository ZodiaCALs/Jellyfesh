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
    };

    /// <summary>Detail pages select their section in the pane.</summary>
    private static readonly Dictionary<Type, string> Sections = new()
    {
        [typeof(AlbumPage)] = "albums",
        [typeof(ArtistPage)] = "artists",
        [typeof(PlaylistPage)] = "playlists",
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
    private bool _wide;

    private bool _compact;
    private Windows.Graphics.SizeInt32? _fullSize;
    private bool _themeApplied;
    private bool _syncingNav;
    private Notification? _shownNotice;

    public MainWindow(ICoreClient core)
    {
        // The view model exists before the XAML, so the controls it creates can bind to it.
        var queue = DispatcherQueue.GetForCurrentThread();
        var l = new ResourceLocalizer();
        _platform = new ShellPlatform(this, l);
        _vm = new AppViewModel(
            core,
            new WinUiDispatcher(queue),
            l,
            _platform,
            PortDrawingTime.Instance,
            CoreClock.Calibrate(core.MonotonicNowNs),
            new DispatcherOneShotTimer(queue));
        AppHost.Set(_vm, _platform, this);

        InitializeComponent();

        // Shadows of the library (covers, groups, the player bar) fall on the field (ThemeShadow receivers).
        Elevation.AddReceiver(Elevation.Library, Field);

        Title = "Jellyfesh";
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        _hwnd = WindowNative.GetWindowHandle(this);
        PlaceWindow();

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

        _ui.ColorValuesChanged += (_, _) => queue.TryEnqueue(ReadSystemPreferences);
        TryHook(() => _ui.AnimationsEnabledChanged += (_, _) => queue.TryEnqueue(ReadSystemPreferences));
        TryHook(() => _accessibility.HighContrastChanged += (_, _) => queue.TryEnqueue(ApplyTheme));
        VisibilityChanged += OnVisibilityChanged;
        Closed += OnClosed;
        Root.AddHandler(UIElement.PointerPressedEvent, new PointerEventHandler(OnRootPointerPressed), handledEventsToo: true);

        _vm.Start();
        ReadSystemPreferences();
        _backdrop.Refresh();
        _shell = new WindowShell(_hwnd, _vm.Player, l);
        _shell.SetIcon(_vm.AppIcon);
        _media = new MediaTransport(_hwnd, _vm.Player, _vm.Services, queue);
        _devices = new AudioDeviceNotifications(queue, c => _vm.Services.Send(c));
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
            NowPlaying.PrepareArtworkFrom(_wide ? Bar.ArtElement : Mini.ArtElement);
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
        Nav.Visibility = Visibility.Collapsed;
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
        Nav.Visibility = Visibility.Visible;
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
        if (_wide)
        {
            Bar.Focus(FocusState.Programmatic);
        }
        else
        {
            Mini.Focus(FocusState.Programmatic);
        }
    }

    /// <summary>The player bar replaces the mini player's capsule in windows 900 px and wider.</summary>
    private void OnContentAreaSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var wide = SeekGeometry.UsesPlayerBar(e.NewSize.Width);
        if (wide != _wide)
        {
            _wide = wide;
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

    private static void TryHook(Action hook)
    {
        try
        {
            hook();
        }
        catch (Exception)
        {
            // Not on this Windows version (the minimum is 1809, ADR-0007); live updates are lost only.
        }
    }

    // ---- what draws while it can be seen (Still mode) ----

    /// <summary>
    /// The seek bar and lyrics draw only while Now Playing is open, the mini player's ring only while
    /// it shows, and nothing while the window is minimised or hidden; the ticker then sleeps.
    /// </summary>
    private void UpdateActivity()
    {
        var player = _vm.Player;
        var floating = player.HasTrack && !_nowPlayingOpen && !_compact;
        var mini = floating && !_wide;
        var bar = floating && _wide;
        Mini.Visibility = mini ? Visibility.Visible : Visibility.Collapsed;
        Bar.Visibility = bar ? Visibility.Visible : Visibility.Collapsed;
        ContentFade.Visibility = mini || bar ? Visibility.Visible : Visibility.Collapsed;
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
        ThemeBrushes.SetTransparency(_vm.Settings.Transparency);
        _backdrop.SetReduceEffects(_vm.Settings.ReduceEffects);
        _shell?.SetIcon(_vm.AppIcon);
        TitleIcon.ImageSource = new BitmapImage(AppAssets.IconPng(_vm.AppIcon, 32));
        ApplyCaptionButtons(dark);
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

    // ---- navigation ----

    private void SelectSection(string tag)
    {
        var item = Nav.MenuItems.OfType<NavigationViewItem>().FirstOrDefault(i => (string?)i.Tag == tag);
        if (item is not null && !ReferenceEquals(Nav.SelectedItem, item))
        {
            Nav.SelectedItem = item; // raises SelectionChanged, which navigates
        }
        else if (TopPages.TryGetValue(tag, out var page))
        {
            Navigate(page);
        }
    }

    private void OnNavSelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (_syncingNav)
        {
            return;
        }

        if (args.IsSettingsSelected)
        {
            Navigate(typeof(SettingsPage));
        }
        else if (args.SelectedItemContainer?.Tag is string tag && TopPages.TryGetValue(tag, out var page))
        {
            Navigate(page);
        }
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
        _syncingNav = true;
        try
        {
            if (e.SourcePageType == typeof(SettingsPage))
            {
                Nav.SelectedItem = Nav.SettingsItem;
            }
            else
            {
                var tag = TopPages.FirstOrDefault(p => p.Value == e.SourcePageType).Key ?? Sections.GetValueOrDefault(e.SourcePageType);
                var item = Nav.MenuItems.OfType<NavigationViewItem>().FirstOrDefault(i => (string?)i.Tag == tag);
                if (item is not null)
                {
                    Nav.SelectedItem = item;
                }
            }
        }
        finally
        {
            _syncingNav = false;
        }
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
        else if (ContentFrame.Content is SourcesPage sources && sources.GoUp())
        {
            // Up a level of the browsed source, then back to the list of sources.
        }
        else if (ContentFrame.CanGoBack)
        {
            ContentFrame.GoBack();
        }
    }

    private void OnBackRequested(TitleBar sender, object args) => GoBack();

    private void OnPaneToggleRequested(TitleBar sender, object args) => Nav.IsPaneOpen = !Nav.IsPaneOpen;

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
        _media?.Dispose();
        _devices?.Dispose();
        _shell?.Dispose();
        _vm.Dispose(); // saves the session and shuts the core down
    }
}
