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
    public PlayerBar()
    {
        Shell = AppHost.App;
        Player = Shell.Player;
        InitializeComponent();
        Player.BarProgress.PropertyChanged += OnProgressChanged;
        Seek.SeekRequested += OnSeekRequested;
        Seek.ScrubMoved += OnScrubMoved;
        Seek.StepsChanged += (_, steps) => Player.BarProgress.Steps = steps;
    }

    public AppViewModel Shell { get; }

    public PlayerViewModel Player { get; }

    /// <summary>The artwork, where Now Playing's artwork grows from (a connected animation).</summary>
    public UIElement ArtElement => Art;

    /// <summary>The person asked for Now Playing (and which panel).</summary>
    public event EventHandler<NowPlayingPanel>? OpenRequested;

    private void OnOpenClick(object sender, RoutedEventArgs e) => OpenRequested?.Invoke(this, NowPlayingPanel.Default);

    private void OnLyricsClick(object sender, RoutedEventArgs e) => OpenRequested?.Invoke(this, NowPlayingPanel.Lyrics);

    private void OnQueueClick(object sender, RoutedEventArgs e) => OpenRequested?.Invoke(this, NowPlayingPanel.Queue);

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
