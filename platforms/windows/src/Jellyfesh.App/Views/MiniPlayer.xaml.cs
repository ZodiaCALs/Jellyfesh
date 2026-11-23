using System.ComponentModel;
using Jellyfesh.App.Shell;
using Jellyfesh.Core;
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

    /// <summary>
    /// Where the capsule sits for the look's bottom-player style (ADR-0020), as in Jellyfesh Web: as
    /// wide as it may be up to 560 epx, centred; Compact as a bar as wide as the player bar; Floating
    /// 360 epx at the end corner. <paramref name="available"/> is the room the dock has.
    /// </summary>
    public void Place(MiniPlayerStyle style, double available)
    {
        HorizontalAlignment = style == MiniPlayerStyle.Floating ? HorizontalAlignment.Right : HorizontalAlignment.Stretch;
        Width = style == MiniPlayerStyle.Floating ? Math.Max(0, Math.Min(FloatingWidth, available)) : double.NaN;
        Capsule.MaxWidth = style switch
        {
            MiniPlayerStyle.Compact => 1280,
            MiniPlayerStyle.Floating => double.PositiveInfinity,
            _ => 560,
        };
    }

    /// <summary>The Floating style's capsule (the web's 360 px).</summary>
    private const double FloatingWidth = 360;

    private void OnCapsuleSizeChanged(object sender, SizeChangedEventArgs e)
    {
        UpdateSteps();
        DrawProgress();
    }

    private void DrawProgress() =>
        ProgressLine.Width = SeekGeometry.MiniProgressLength(Player.MiniProgress.Fraction, Capsule.ActualWidth);
}
