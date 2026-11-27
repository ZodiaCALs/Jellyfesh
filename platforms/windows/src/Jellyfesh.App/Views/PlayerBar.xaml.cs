using System.ComponentModel;
using Jellyfesh.App.Shell;
using Jellyfesh.Drawing;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace Jellyfesh.App.Views;

/// <summary>Which Now Playing panel the player bar asked for.</summary>
public enum NowPlayingPanel
{
    Default,
    Lyrics,
    Queue,
}

public sealed partial class PlayerBar : UserControl
{
    public static readonly DependencyProperty ShowsVolumeLineProperty =
        DependencyProperty.Register(nameof(ShowsVolumeLine), typeof(bool), typeof(PlayerBar), new PropertyMetadata(true, (d, _) => ((PlayerBar)d).Fit()));

    public static readonly DependencyProperty FitsVolumeLineProperty =
        DependencyProperty.Register(nameof(FitsVolumeLine), typeof(bool), typeof(PlayerBar), new PropertyMetadata(true));

    public static readonly DependencyProperty FitsPanesProperty =
        DependencyProperty.Register(nameof(FitsPanes), typeof(bool), typeof(PlayerBar), new PropertyMetadata(true));

    public static readonly DependencyProperty FitsMuteProperty =
        DependencyProperty.Register(nameof(FitsMute), typeof(bool), typeof(PlayerBar), new PropertyMetadata(true));

    public PlayerBar()
    {
        Shell = AppHost.App;
        Player = Shell.Player;
        InitializeComponent();
        Player.BarProgress.PropertyChanged += OnProgressChanged;
        Seek.SeekRequested += OnSeekRequested;
        Seek.ScrubMoved += OnScrubMoved;
        Seek.StepsChanged += (_, steps) => Player.BarProgress.Steps = steps;

        // The lyrics bar: its line, and the sweep of the word being sung.
        var bar = Shell.LyricsBar;
        bar.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(LyricsBarPresenter.Line))
            {
                BarLine.Line = bar.Line;
                BarLine.ApplySweep(bar.Sweep);
            }
        };
        bar.SweepChanged += BarLine.ApplySweep;

        // The look's own highlight colour, if it chose one (ADR-0020).
        BarLine.Highlight = Controls.LyricsLook.BarHighlight(Shell.Look);
        Shell.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(AppViewModel.Look))
            {
                BarLine.Highlight = Controls.LyricsLook.BarHighlight(Shell.Look);
            }
        };
        Loaded += (_, _) => bar.Show(true);
        Unloaded += (_, _) => bar.Show(false);
    }

    public AppViewModel Shell { get; }

    public PlayerViewModel Player { get; }

    /// <summary>The artwork, where Now Playing's artwork grows from (a connected animation).</summary>
    public UIElement ArtElement => Art;

    /// <summary>The person asked for Now Playing (and which panel).</summary>
    public event EventHandler<NowPlayingPanel>? OpenRequested;

    /// <summary>The person asked for the mini player (the bar's button).</summary>
    public event EventHandler? MiniPlayerRequested;

    /// <summary>The volume line beside the mute button; narrow windows leave it out (the web's 1040 px).</summary>
    public bool ShowsVolumeLine
    {
        get => (bool)GetValue(ShowsVolumeLineProperty);
        set => SetValue(ShowsVolumeLineProperty, value);
    }

    /// <summary>The volume line shows: the window allows it and the bar has room for it (PlayerBarLayout).</summary>
    public bool FitsVolumeLine
    {
        get => (bool)GetValue(FitsVolumeLineProperty);
        private set => SetValue(FitsVolumeLineProperty, value);
    }

    /// <summary>Lyrics and Queue show: the bar has room for them (Now Playing has both).</summary>
    public bool FitsPanes
    {
        get => (bool)GetValue(FitsPanesProperty);
        private set => SetValue(FitsPanesProperty, value);
    }

    /// <summary>The mute button shows: the bar has room for it.</summary>
    public bool FitsMute
    {
        get => (bool)GetValue(FitsMuteProperty);
        private set => SetValue(FitsMuteProperty, value);
    }

    private void OnOpenClick(object sender, RoutedEventArgs e) => OpenRequested?.Invoke(this, NowPlayingPanel.Default);

    private void OnLyricsClick(object sender, RoutedEventArgs e) => OpenRequested?.Invoke(this, NowPlayingPanel.Lyrics);

    private void OnQueueClick(object sender, RoutedEventArgs e) => OpenRequested?.Invoke(this, NowPlayingPanel.Queue);

    /// <summary>The quality menu: the format line, and Streaming Quality's levels or why there are none.</summary>
    private void OnQualityClick(object sender, RoutedEventArgs e) => PlayerMenus.ShowQuality(QualityBadge, Player, Shell.Services);

    /// <summary>Devices, while a remote source plays.</summary>
    private void OnDeviceClick(object sender, RoutedEventArgs e) => PlayerMenus.ShowDevices(DeviceButton, Player, Shell.Services);

    private void OnMiniPlayerClick(object sender, RoutedEventArgs e) => MiniPlayerRequested?.Invoke(this, EventArgs.Empty);

    /// <summary>More: Open in {Service}, Stop Controlling, Desktop Lyrics.</summary>
    private void OnMoreClick(object sender, RoutedEventArgs e) => PlayerMenus.ShowMore(MoreButton, Player, Shell.Services);

    private void OnBarSizeChanged(object sender, SizeChangedEventArgs e) => Fit();

    private void OnToolsSizeChanged(object sender, SizeChangedEventArgs e) => Fit();

    /// <summary>
    /// Every part keeps to its own column (v0.0.9): what a bar this wide holds, and the middle
    /// column's width beside the tools; the title's column takes the rest. Settles in one more
    /// layout pass when the tools change width.
    /// </summary>
    private void Fit()
    {
        var width = Root?.ActualWidth ?? 0;
        if (width <= 0)
        {
            return;
        }

        var parts = PlayerBarLayout.For(width);
        FitsVolumeLine = ShowsVolumeLine && parts.VolumeLine;
        FitsPanes = parts.Panes;
        FitsMute = parts.Mute;
        var middle = PlayerBarLayout.MiddleWidth(width, Tools.ActualWidth);
        if (Math.Abs(MiddleColumn.Width.Value - middle) >= 0.5)
        {
            MiddleColumn.Width = new GridLength(middle);
        }
    }

    private void OnProgressChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(ProgressPresenter.Fraction) or nameof(ProgressPresenter.DurationText))
        {
            SyncSeek();
        }
    }

    /// <summary>The bar shows the presenter's position, except while the person drags it.</summary>
    private void SyncSeek()
    {
        if (Seek.IsDragging)
        {
            return;
        }

        var progress = Player.BarProgress;
        Seek.Maximum = progress.DurationUs / 1_000_000.0;
        Seek.Value = Math.Min(Seek.Maximum, progress.PositionUs / 1_000_000.0);
    }

    private void OnScrubMoved(object? sender, double seconds)
    {
        Player.BarProgress.IsScrubbing = true;
        Player.BarProgress.Show((long)(seconds * 1_000_000));
    }

    private void OnSeekRequested(object? sender, double seconds)
    {
        Player.BarProgress.IsScrubbing = false;
        Player.SeekToUs((long)(seconds * 1_000_000));
    }

    private void OnVolumeChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        // The binding's own updates match the view model; only the person's moves are sent.
        if (Math.Abs(e.NewValue - Player.Volume) >= 0.5)
        {
            Player.SetVolumeFromUser(e.NewValue);
        }
    }
}
