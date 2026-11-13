using System.ComponentModel;
using Jellyfesh.App.Shell;
using Jellyfesh.Drawing;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Views;

public sealed partial class MiniPlayer : UserControl
{
    public MiniPlayer()
    {
        Shell = AppHost.App;
        Player = Shell.Player;
        InitializeComponent();
        Player.MiniProgress.PropertyChanged += OnProgressChanged;
    }

    public AppViewModel Shell { get; }

    public PlayerViewModel Player { get; }

    /// <summary>The artwork, where Now Playing's artwork grows from (a connected animation).</summary>
    public UIElement ArtElement => Art;

    /// <summary>The person asked for Now Playing.</summary>
    public event EventHandler? OpenRequested;

    private void OnOpenClick(object sender, RoutedEventArgs e) => OpenRequested?.Invoke(this, EventArgs.Empty);

    private void OnProgressChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(ProgressPresenter.Fraction))
        {
            DrawProgress();
        }
    }

    /// <summary>One visible step per device pixel of the line (the presenter redraws only then).</summary>
    public void UpdateSteps() =>
        Player.MiniProgress.Steps = SeekGeometry.MiniSteps(Capsule.ActualWidth, XamlRoot?.RasterizationScale ?? 1);

    private void OnCapsuleSizeChanged(object sender, SizeChangedEventArgs e)
    {
        UpdateSteps();
        DrawProgress();
    }

    private void DrawProgress() =>
        ProgressLine.Width = SeekGeometry.MiniProgressLength(Player.MiniProgress.Fraction, Capsule.ActualWidth);
}
