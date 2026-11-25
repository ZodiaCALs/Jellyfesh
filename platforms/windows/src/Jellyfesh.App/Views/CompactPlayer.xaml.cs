using System.ComponentModel;
using Jellyfesh.App.Shell;
using Jellyfesh.Drawing;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Views;

public sealed partial class CompactPlayer : UserControl
{
    public CompactPlayer()
    {
        Shell = AppHost.App;
        Player = Shell.Player;
        InitializeComponent();
        Player.MiniProgress.PropertyChanged += OnProgressChanged;

        // The lyrics bar's line (Show Lyrics Line), with the sweep of the word being sung.
        var bar = Shell.LyricsBar;
        bar.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(LyricsBarPresenter.Line))
            {
                LyricLine.Line = bar.Line;
                LyricLine.ApplySweep(bar.Sweep);
            }
        };
        bar.SweepChanged += LyricLine.ApplySweep;
        LyricLine.Highlight = Controls.LyricsLook.BarHighlight(Shell.Look);
        Shell.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(AppViewModel.Look))
            {
                LyricLine.Highlight = Controls.LyricsLook.BarHighlight(Shell.Look);
            }
        };
    }

    private bool _shown;

    public AppViewModel Shell { get; }

    public PlayerViewModel Player { get; }

    /// <summary>The element that moves the window while it is compact.</summary>
    public UIElement DragRegion => DragArea;

    /// <summary>The person asked for the full window again.</summary>
    public event EventHandler? ExpandRequested;

    private void OnExpandClick(object sender, RoutedEventArgs e) => ExpandRequested?.Invoke(this, EventArgs.Empty);

    /// <summary>The mini player came on screen or left it: the lyrics bar's line runs only while it shows.</summary>
    public void SetShown(bool shown)
    {
        if (shown == _shown)
        {
            return;
        }

        _shown = shown;
        Shell.LyricsBar.Show(shown);
        if (shown)
        {
            LyricLine.Line = Shell.LyricsBar.Line;
            LyricLine.ApplySweep(Shell.LyricsBar.Sweep);
        }
    }

    /// <summary>One visible step per device pixel of the line (the presenter redraws only then).</summary>
    public void UpdateSteps() =>
        Player.MiniProgress.Steps = SeekGeometry.LineSteps(Line.ActualWidth, XamlRoot?.RasterizationScale ?? 1);

    private void OnProgressChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(ProgressPresenter.Fraction) && Visibility == Visibility.Visible)
        {
            DrawProgress();
        }
    }

    private void OnLineSizeChanged(object sender, SizeChangedEventArgs e)
    {
        UpdateSteps();
        DrawProgress();
    }

    private void DrawProgress() =>
        ProgressLine.Width = SeekGeometry.LineLength(Player.MiniProgress.Fraction, Line.ActualWidth);
}
