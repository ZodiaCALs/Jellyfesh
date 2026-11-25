using System.ComponentModel;
using Jellyfesh.App.Controls;
using Jellyfesh.App.Shell;
using Jellyfesh.App.Theming;
using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Foundation;

namespace Jellyfesh.App.Views;

public sealed partial class NowPlayingView : UserControl
{
    private const double CompactWidth = 760;
    private const double MediumWidth = 1100;

    /// <summary>Now Playing's artwork on wide windows (Luminous).</summary>
    private const double ArtworkSize = 380;

    /// <summary>The look's Artwork layout: the large cover on wide and medium windows.</summary>
    private const double CoverSize = 520;
    private const double MediumCoverSize = 440;

    /// <summary>The look's Lyrics layout: the small cover beside the titles, above the pane.</summary>
    private const double SmallCoverSize = 72;

    private Image _front;
    private Image _back;
    private string? _backdropPath = string.Empty;
    private bool _open;
    private bool _compact;
    private bool _medium;
    private double _width = 1200;
    private bool _widePane;
    private bool _sungShown;
    private Storyboard? _transition;
    private Storyboard? _backdropFade;
    private bool _artworkFromSource;

    /// <summary>The key of the artwork's connected animation (mini player or player bar to here).</summary>
    private const string ArtworkAnimation = "jf-artwork";

    public NowPlayingView()
    {
        Shell = AppHost.App;
        Player = Shell.Player;
        Comments = Shell.Comments;
        Comments.PropertyChanged += (_, e) =>
        {
            // The song's source has no comments: back to the lyrics.
            if (e.PropertyName == nameof(CommentsViewModel.IsAvailable) && !Comments.IsAvailable && CommentsTab?.IsChecked == true)
            {
                LyricsTab.IsChecked = true;
            }
        };
        InitializeComponent();
        _front = BackdropA;
        _back = BackdropB;
        Player.Progress.PropertyChanged += OnProgressChanged;
        Player.PropertyChanged += OnPlayerChanged;

        // The stage's shadows (the artwork's shadow-3) fall on its backdrop (ThemeShadow receivers).
        Elevation.AddReceiver(Elevation.Stage, MistyBackdrop);
        Elevation.AddReceiver(Elevation.Stage, BackdropA);
        Elevation.AddReceiver(Elevation.Stage, BackdropB);
        Seek.SeekRequested += OnSeekRequested;
        Seek.ScrubMoved += OnScrubMoved;
        Seek.StepsChanged += (_, steps) => Player.Progress.Steps = steps;

        // The look's Artwork layout shows the line being sung under the cover (the lyrics bar's line).
        var bar = Shell.LyricsBar;
        bar.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(LyricsBarPresenter.Line))
            {
                SungLine.Line = bar.Line;
                SungLine.ApplySweep(bar.Sweep);
            }
        };
        bar.SweepChanged += SungLine.ApplySweep;
        SungLine.Line = bar.Line;
        SungLine.Highlight = LyricsLook.BarHighlight(Shell.Look);

        // The look's Now Playing layout, live (ADR-0020).
        Shell.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(AppViewModel.Look))
            {
                SungLine.Highlight = LyricsLook.BarHighlight(Shell.Look);
                UpdatePanels();
            }
        };
    }

    public AppViewModel Shell { get; }

    public PlayerViewModel Player { get; }

    /// <summary>The Comments pane (sources with comments).</summary>
    public CommentsViewModel Comments { get; }

    /// <summary>Whether the lyrics are on screen (the lyrics presenter draws only then).</summary>
    public bool ShowsLyrics => _open && SidePanel.Visibility == Visibility.Visible && LyricsPanel.Visibility == Visibility.Visible;

    /// <summary>The person asked to go back to the library.</summary>
    public event EventHandler? CloseRequested;

    /// <summary>The visible panels changed (lyrics shown or hidden).</summary>
    public event EventHandler? ViewsChanged;

    /// <summary>The person asked for the compact mini player.</summary>
    public event EventHandler? CompactRequested;

    public void Open(bool animate, NowPlayingPanel panel = NowPlayingPanel.Default)
    {
        _open = true;
        Visibility = Visibility.Visible;
        ShowPanel(panel);
        ApplyTheme();
        SyncSeek();
        ScaleArtwork(animate: false);
        Animate(opening: true, animate);
        StartArtworkAnimation(animate);
        PlayButton.Focus(FocusState.Programmatic);
        LyricsPanel.Refresh();
    }

    /// <summary>Shows the lyrics or the queue beside the player (the player bar's buttons).</summary>
    public void ShowPanel(NowPlayingPanel panel)
    {
        switch (panel)
        {
            case NowPlayingPanel.Lyrics:
                LyricsTab.IsChecked = true;
                break;
            case NowPlayingPanel.Queue:
                QueueTab.IsChecked = true;
                break;
        }
    }

    /// <summary>The artwork will grow from <paramref name="source"/> when Now Playing opens, where the toolkit allows.</summary>
    public void PrepareArtworkFrom(UIElement source)
    {
        try
        {
            ConnectedAnimationService.GetForCurrentView().PrepareToAnimate(ArtworkAnimation, source);
            _artworkFromSource = true;
        }
        catch (Exception e) when (e is InvalidOperationException or ArgumentException or System.Runtime.InteropServices.COMException)
        {
            _artworkFromSource = false; // the slide and fade alone
        }
    }

    private void StartArtworkAnimation(bool animate)
    {
        if (!_artworkFromSource)
        {
            return;
        }

        _artworkFromSource = false;
        try
        {
            var animation = ConnectedAnimationService.GetForCurrentView().GetAnimation(ArtworkAnimation);
            if (animation is null)
            {
                return;
            }

            if (animate && PlayerScroller.Visibility == Visibility.Visible)
            {
                animation.Configuration = new BasicConnectedAnimationConfiguration();
                animation.TryStart(ArtFrame);
            }
            else
            {
                animation.Cancel();
            }
        }
        catch (Exception e) when (e is InvalidOperationException or ArgumentException or System.Runtime.InteropServices.COMException)
        {
            // Not available here: the slide and fade alone.
        }
    }

    /// <summary>The artwork rests at 88 % while paused and springs back when playing (dur-3 Spring); full size with reduced motion.</summary>
    private void ScaleArtwork(bool animate)
    {
        var scale = (float)NowPlayingMotion.ArtworkScale(Player.IsPlaying, Shell.ReduceMotion);
        var duration = animate && _open && !Shell.ReduceMotion ? TokenValues.Duration(3) : TimeSpan.Zero;
        Motion.Scale(ArtFrame, scale, duration, TokenValues.Spring);
    }

    private void OnArtSizeChanged(object sender, SizeChangedEventArgs e) =>
        ArtFrame.CenterPoint = new System.Numerics.Vector3((float)e.NewSize.Width / 2, (float)e.NewSize.Height / 2, 0);

    public void Close(bool animate)
    {
        _open = false;
        Animate(opening: false, animate);
    }

    /// <summary>The style (Artwork or Misty) and the backdrop for the current theme.</summary>
    public void ApplyTheme()
    {
        var misty = Shell.Settings.Style == BackdropStyle.Misty;
        ApplyFrame();

        var path = misty ? null : Shell.BackdropPath;
        if (path != _backdropPath)
        {
            _backdropPath = path;
            ShowBackdrop(path, animate: _open && !Shell.ReduceMotion);
        }
    }

    // ---- backdrop ----

    /// <summary>Crossfades to the baked backdrop over dur-4 with the Fluid curve; blur never animates (MAT-10).</summary>
    private void ShowBackdrop(string? path, bool animate)
    {
        // A fade still running (a quick skip) ends where it is; its completion must not clear the new image.
        _backdropFade?.Stop();
        (_front, _back) = (_back, _front);
        _front.Source = path is null ? null : new BitmapImage(new Uri(path));
        var storyboard = new Storyboard();
        storyboard.Children.Add(Fade(_front, path is null ? 0 : 1, animate ? TokenValues.Duration(4) : TimeSpan.Zero, TokenValues.Fluid));
        storyboard.Children.Add(Fade(_back, 0, animate ? TokenValues.Duration(4) : TimeSpan.Zero, TokenValues.Fluid));
        var old = _back;
        storyboard.Completed += (_, _) =>
        {
            if (ReferenceEquals(old, _back))
            {
                old.Source = null; // free the decoded image that faded out
            }
        };
        _backdropFade = storyboard;
        storyboard.Begin();
    }

    private static DoubleAnimationUsingKeyFrames Fade(DependencyObject target, double to, TimeSpan duration, CubicBezierCurve curve)
    {
        var animation = new DoubleAnimationUsingKeyFrames();
        animation.KeyFrames.Add(Key(to, duration, curve));
        Storyboard.SetTarget(animation, target);
        Storyboard.SetTargetProperty(animation, "Opacity");
        return animation;
    }

    private static SplineDoubleKeyFrame Key(double value, TimeSpan at, CubicBezierCurve curve) => new()
    {
        KeyTime = KeyTime.FromTimeSpan(at),
        Value = value,
        KeySpline = new KeySpline { ControlPoint1 = new Point(curve.X1, curve.Y1), ControlPoint2 = new Point(curve.X2, curve.Y2) },
    };

    /// <summary>Opens with Misty's Deeper motion (320 ms) and closes with Back (300 ms); reduced motion completes at once (MOT-12).</summary>
    private void Animate(bool opening, bool animate)
    {
        _transition?.Stop();
        if (!animate)
        {
            Surface.Opacity = 1;
            Slide.Y = 0;
            Visibility = opening ? Visibility.Visible : Visibility.Collapsed;
            ViewsChanged?.Invoke(this, EventArgs.Empty);
            return;
        }

        // Under reduced motion only a short fade (dur-1): nothing slides.
        var reduce = Shell.ReduceMotion;
        if (opening)
        {
            Surface.Opacity = 0;
            Slide.Y = reduce ? 0 : 32;
        }

        var duration = reduce ? TokenValues.Duration(1) : TimeSpan.FromMilliseconds(opening ? 320 : 300);
        var curve = opening || reduce ? TokenValues.Fluid : TokenValues.Exit;
        var storyboard = new Storyboard();
        storyboard.Children.Add(Fade(Surface, opening ? 1 : 0, duration, curve));
        if (!reduce)
        {
            var slide = new DoubleAnimationUsingKeyFrames();
            slide.KeyFrames.Add(Key(opening ? 0 : 24, duration, curve));
            Storyboard.SetTarget(slide, Slide);
            Storyboard.SetTargetProperty(slide, "Y");
            storyboard.Children.Add(slide);
        }
        storyboard.Completed += (_, _) =>
        {
            if (!_open)
            {
                Visibility = Visibility.Collapsed;
            }

            Surface.Opacity = 1;
            Slide.Y = 0;
        };
        _transition = storyboard;
        storyboard.Begin();
        ViewsChanged?.Invoke(this, EventArgs.Empty);
    }

    // ---- seek bar ----

    private void OnProgressChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(Jellyfesh.Drawing.ProgressPresenter.Fraction) or nameof(Jellyfesh.Drawing.ProgressPresenter.DurationText))
        {
            SyncSeek();
        }
    }

    /// <summary>The seek bar shows the presenter's position, except while the person drags it.</summary>
    private void SyncSeek()
    {
        if (Seek.IsDragging)
        {
            return;
        }

        var progress = Player.Progress;
        Seek.Maximum = progress.DurationUs / 1_000_000.0;
        Seek.Value = Math.Min(Seek.Maximum, progress.PositionUs / 1_000_000.0);
    }

    private void OnScrubMoved(object? sender, double seconds)
    {
        Player.Progress.IsScrubbing = true;
        Player.Progress.Show((long)(seconds * 1_000_000));
    }

    private void OnSeekRequested(object? sender, double seconds)
    {
        Player.Progress.IsScrubbing = false;
        Player.SeekToUs((long)(seconds * 1_000_000));
    }

    // ---- volume and output ----

    private void OnVolumeChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        // The binding's own updates match the view model; only the person's moves are sent.
        if (Math.Abs(e.NewValue - Player.Volume) >= 0.5)
        {
            Player.SetVolumeFromUser(e.NewValue);
        }
    }

    /// <summary>The output devices, or Devices in a remote session (it asks the service for its devices).</summary>
    private void OnDeviceClick(object sender, RoutedEventArgs e) => PlayerMenus.ShowDevices(DeviceButton, Player, Shell.Services);

    /// <summary>The quality menu, as from the player bar's badge.</summary>
    private void OnQualityClick(object sender, RoutedEventArgs e) => PlayerMenus.ShowQuality(QualityBadge, Player, Shell.Services);

    /// <summary>More: Open in {Service}, Stop Controlling.</summary>
    private void OnMoreClick(object sender, RoutedEventArgs e) => PlayerMenus.ShowMore(MoreButton, Player, Shell.Services);

    // ---- panels ----

    private void OnViewChecked(object sender, RoutedEventArgs e) => UpdatePanels();

    private void OnSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var width = e.NewSize.Width;
        _width = width;
        _compact = width < CompactWidth;
        var medium = width < MediumWidth;
        _medium = medium;
        Body.Padding = _compact ? new Thickness(24, 12, 24, 12) : medium ? new Thickness(32, 20, 32, 20) : new Thickness(64, 24, 48, 24);
        Body.ColumnSpacing = _compact ? 0 : medium ? 32 : 56;
        ArrangeDock(_compact, medium);
        PlayerTab.Visibility = _compact ? Visibility.Visible : Visibility.Collapsed;
        if (!_compact && PlayerTab.IsChecked == true)
        {
            LyricsTab.IsChecked = true;
        }

        UpdatePanels();
    }

    /// <summary>
    /// The dock in one row (wide), or the scrubber above the transport, the device button and More
    /// without the volume (narrow windows, where one row would not fit).
    /// </summary>
    private void ArrangeDock(bool narrow, bool medium)
    {
        Grid.SetColumn(ScrubRow, narrow ? 0 : 1);
        Grid.SetColumnSpan(ScrubRow, narrow ? 5 : 1);
        Grid.SetRow(Transport, narrow ? 1 : 0);
        Grid.SetColumnSpan(Transport, narrow ? 3 : 1);
        Transport.HorizontalAlignment = narrow ? HorizontalAlignment.Center : HorizontalAlignment.Left;
        Grid.SetRow(DeviceButton, narrow ? 1 : 0);
        Grid.SetRow(MoreButton, narrow ? 1 : 0);
        VolumeGroup.Visibility = narrow ? Visibility.Collapsed : Visibility.Visible;
        VolumeSlider.Width = medium ? 100 : 140;
        DockGrid.ColumnSpacing = narrow ? 8 : medium ? 14 : 20;
        Dock.CornerRadius = new CornerRadius(narrow ? TokenValues.Number("Radius.2xl") : TokenValues.Number("Radius.Full"));
        Dock.Margin = narrow ? new Thickness(12, 0, 12, 12) : medium ? new Thickness(32, 0, 32, 20) : new Thickness(64, 0, 64, 24);
    }

    /// <summary>
    /// Wide: the player and one side panel, arranged by the look's Now Playing layout (ADR-0020):
    /// side by side (Stage), a large cover with the sung line instead of the Lyrics pane (Artwork),
    /// or the pane across the window under a small cover (Lyrics). Compact: one of the four at a time.
    /// </summary>
    private void UpdatePanels()
    {
        if (LyricsPanel is null || PlayerScroller is null)
        {
            return; // during InitializeComponent
        }

        var arrangement = _compact ? NowPlayingArrangement.Stage : LookRules.Arrangement(Shell.Look.NowPlaying, LyricsTab.IsChecked == true);
        var cover = arrangement == NowPlayingArrangement.Cover;
        var widePane = arrangement == NowPlayingArrangement.WidePane;
        var showPlayer = !_compact || PlayerTab.IsChecked == true;
        var showSide = (!_compact || PlayerTab.IsChecked != true) && !cover;
        PlayerScroller.Visibility = showPlayer ? Visibility.Visible : Visibility.Collapsed;
        SidePanel.Visibility = showSide ? Visibility.Visible : Visibility.Collapsed;
        PlayerColumn.Width = _compact ? new GridLength(showPlayer ? 1 : 0, GridUnitType.Star) : cover ? new GridLength(1, GridUnitType.Star) : GridLength.Auto;
        SideColumn.Width = (_compact && showPlayer) || cover ? new GridLength(0) : new GridLength(1, GridUnitType.Star);
        Grid.SetColumn(SidePanel, _compact || widePane ? 0 : 1);
        ArrangePlayer(cover, widePane);

        var lyricsWereShown = LyricsPanel.Visibility == Visibility.Visible && SidePanel.Visibility == Visibility.Visible;
        LyricsPanel.Visibility = LyricsTab.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        QueuePanel.Visibility = QueueTab.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        InfoPanel.Visibility = InfoTab.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        CommentsPanel.Visibility = CommentsTab.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        if (CommentsTab.IsChecked == true)
        {
            Comments.Open();
        }
        ViewsChanged?.Invoke(this, EventArgs.Empty);
        if (ShowsLyrics && !lyricsWereShown)
        {
            LyricsPanel.Refresh();
        }
    }

    /// <summary>The player for the arrangement: beside the pane, a large cover alone with the sung line, or a row above the pane.</summary>
    private void ArrangePlayer(bool cover, bool widePane)
    {
        Grid.SetRowSpan(PlayerScroller, widePane ? 1 : 2);
        Grid.SetColumnSpan(PlayerScroller, widePane ? 2 : 1);
        Grid.SetRow(SidePanel, widePane ? 1 : 0);
        Grid.SetRowSpan(SidePanel, widePane ? 1 : 2);
        Grid.SetColumnSpan(SidePanel, widePane ? 2 : 1);
        Body.RowSpacing = widePane ? 16 : 0;

        // The artwork is 380 epx on wide windows (Luminous), smaller as the window narrows; the
        // Artwork layout's cover is larger, the Lyrics layout's a small one beside the titles.
        PlayerPanel.Orientation = widePane ? Orientation.Horizontal : Orientation.Vertical;
        PlayerPanel.Width = widePane ? double.NaN
            : cover ? Math.Min(_medium ? MediumCoverSize : CoverSize, Math.Max(240, _width - 96))
            : _compact ? Math.Min(ArtworkSize, Math.Max(240, _width - 48)) : _medium ? 320 : ArtworkSize;
        ArtFrame.Width = ArtFrame.Height = widePane ? SmallCoverSize : double.NaN;
        TitleRow.VerticalAlignment = widePane ? VerticalAlignment.Center : VerticalAlignment.Stretch;
        if (widePane != _widePane)
        {
            _widePane = widePane;
            ApplyFrame();
        }

        SungLine.Visibility = cover ? Visibility.Visible : Visibility.Collapsed;
        if (cover != _sungShown)
        {
            _sungShown = cover;
            Shell.LyricsBar.Show(cover);
        }
    }

    /// <summary>
    /// Misty: the artwork as hero media on a Clear frame. Artwork: the artwork alone, on shadow-3
    /// (Luminous). The Lyrics layout's small cover has no frame and a smaller radius.
    /// </summary>
    private void ApplyFrame()
    {
        var misty = Shell.Settings.Style == BackdropStyle.Misty && !_widePane;
        ArtFrame.Padding = new Thickness(misty ? 12 : 0);
        ArtFrame.BorderThickness = new Thickness(misty ? 1 : 0);
        ArtFrame.Background = misty ? (Brush)Application.Current.Resources["JfGlassClear"] : null;
        ArtBorder.CornerRadius = new CornerRadius(TokenValues.Number(_widePane ? "Artwork.RadiusGrid" : "Artwork.RadiusHero"));
        Elevation.Cast(ArtBorder, misty ? 0 : Elevation.Shadow3, Elevation.Stage);
    }

    private void OnPlayerChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(PlayerViewModel.IsPlaying))
        {
            ScaleArtwork(animate: true);
        }
    }

    private void OnBackClick(object sender, RoutedEventArgs e) => CloseRequested?.Invoke(this, EventArgs.Empty);

    private void OnCompactClick(object sender, RoutedEventArgs e) => CompactRequested?.Invoke(this, EventArgs.Empty);

    private void OnEscape(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (_open)
        {
            CloseRequested?.Invoke(this, EventArgs.Empty);
            args.Handled = true;
        }
    }

    /// <summary>Get Info for what plays: the core's details, with what the player knows meanwhile.</summary>
    private void OnGetInfoClick(object sender, RoutedEventArgs e)
    {
        var services = AppHost.Services;
        if (services.Hub.State.NowPlaying is not { } np || ItemRef.Of(np.Track) is not { } item)
        {
            return;
        }

        var source = np.Track is TrackRef.Source s ? ItemLinks.ServiceName(services.Hub.State.Sources, s.Track.Plugin) : null;
        var known = new KnownDetails(np.Title, np.Artist, np.Album, null, np.DurationMs, np.Path, source);
        _ = InfoDialog.ShowAsync(AppHost.Platform, new DetailsViewModel(services, item, InfoKind.Song, known), services.L);
    }

    /// <summary>More comments at the end of the list.</summary>
    private void OnCommentsScrolled(object? sender, ScrollViewerViewChangedEventArgs e)
    {
        if (CommentsPanel.VerticalOffset >= CommentsPanel.ScrollableHeight - 200)
        {
            Comments.LoadMoreCommand.Execute(null);
        }
    }
}
