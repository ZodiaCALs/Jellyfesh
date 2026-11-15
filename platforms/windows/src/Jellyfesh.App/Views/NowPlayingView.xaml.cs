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

    private Image _front;
    private Image _back;
    private string? _backdropPath = string.Empty;
    private bool _open;
    private bool _compact;
    private Storyboard? _transition;
    private Storyboard? _backdropFade;
    private bool _artworkFromSource;

    /// <summary>The key of the artwork's connected animation (mini player or player bar to here).</summary>
    private const string ArtworkAnimation = "jf-artwork";

    public NowPlayingView()
    {
        Shell = AppHost.App;
        Player = Shell.Player;
        InitializeComponent();
        _front = BackdropA;
        _back = BackdropB;
        Player.Progress.PropertyChanged += OnProgressChanged;
        Player.PropertyChanged += OnPlayerChanged;
        Seek.SeekRequested += OnSeekRequested;
        Seek.ScrubMoved += OnScrubMoved;
        Seek.StepsChanged += (_, steps) => Player.Progress.Steps = steps;
    }

    public AppViewModel Shell { get; }

    public PlayerViewModel Player { get; }

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
        var resources = Application.Current.Resources;

        // Misty: the artwork as hero media on a Clear frame. Artwork: the artwork alone, lifted.
        ArtFrame.Padding = new Thickness(misty ? 12 : 0);
        ArtFrame.BorderThickness = new Thickness(misty ? 1 : 0);
        ArtFrame.Background = misty ? (Brush)resources["JfGlassClear"] : null;
        ArtBorder.Translation = misty ? default : new System.Numerics.Vector3(0, 0, 32);
        ArtBorder.Shadow = misty ? null : new ThemeShadow();

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

    private void OnDevicesOpening(object? sender, object e)
    {
        DevicesMenu.Items.Clear();
        foreach (var device in Player.Devices)
        {
            var item = new RadioMenuFlyoutItem
            {
                Text = device.Name,
                GroupName = "OutputDevices",
                IsChecked = Equals(device, Player.SelectedDevice),
            };
            var choice = device;
            item.Click += (_, _) => Player.SelectedDevice = choice;
            DevicesMenu.Items.Add(item);
        }

        // A good moment to look again: the core never polls for devices (ADR-0009).
        Shell.Services.Send(new CoreCommand.RefreshDevices());
    }

    // ---- panels ----

    private void OnViewChecked(object sender, RoutedEventArgs e) => UpdatePanels();

    private void OnSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var width = e.NewSize.Width;
        _compact = width < CompactWidth;
        var medium = width < MediumWidth;
        PlayerPanel.Width = _compact ? Math.Min(400, Math.Max(240, width - 48)) : medium ? 320 : 400;
        Body.Padding = _compact ? new Thickness(24, 16, 24, 24) : medium ? new Thickness(32, 24, 32, 32) : new Thickness(48, 32, 48, 40);
        Body.ColumnSpacing = _compact ? 0 : medium ? 32 : 56;
        PlayerTab.Visibility = _compact ? Visibility.Visible : Visibility.Collapsed;
        if (!_compact && PlayerTab.IsChecked == true)
        {
            LyricsTab.IsChecked = true;
        }

        UpdatePanels();
    }

    /// <summary>Wide: the player and one side panel. Compact: one of the four at a time.</summary>
    private void UpdatePanels()
    {
        if (LyricsPanel is null || PlayerScroller is null)
        {
            return; // during InitializeComponent
        }

        var showPlayer = !_compact || PlayerTab.IsChecked == true;
        var showSide = !_compact || PlayerTab.IsChecked != true;
        PlayerScroller.Visibility = showPlayer ? Visibility.Visible : Visibility.Collapsed;
        SidePanel.Visibility = showSide ? Visibility.Visible : Visibility.Collapsed;
        PlayerColumn.Width = _compact ? new GridLength(showPlayer ? 1 : 0, GridUnitType.Star) : GridLength.Auto;
        SideColumn.Width = _compact && showPlayer ? new GridLength(0) : new GridLength(1, GridUnitType.Star);
        Grid.SetColumn(SidePanel, _compact ? 0 : 1);

        var lyricsWereShown = LyricsPanel.Visibility == Visibility.Visible && SidePanel.Visibility == Visibility.Visible;
        LyricsPanel.Visibility = LyricsTab.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        QueuePanel.Visibility = QueueTab.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        InfoPanel.Visibility = InfoTab.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        ViewsChanged?.Invoke(this, EventArgs.Empty);
        if (ShowsLyrics && !lyricsWereShown)
        {
            LyricsPanel.Refresh();
        }
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
}
