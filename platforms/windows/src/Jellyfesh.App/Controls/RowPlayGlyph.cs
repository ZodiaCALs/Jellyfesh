using Jellyfesh.App.Shell;
using Jellyfesh.App.Theming;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Jellyfesh.App.Controls;

/// <summary>
/// The play button over a row's artwork (docs/ui.md, "Rows and cards"; the web's ItemRow): the
/// play glyph on the scrim while the row is pointed at or has keyboard focus. It is the row's own:
/// a click passes through to the row, which plays from there as choosing it (a click, Enter) does,
/// and Narrator reads the row. A row that cannot play here shows none.
/// </summary>
public sealed partial class RowPlayGlyph : Grid
{
    public static readonly DependencyProperty IsPlayableProperty =
        DependencyProperty.Register(nameof(IsPlayable), typeof(bool), typeof(RowPlayGlyph), new PropertyMetadata(true, (d, _) => ((RowPlayGlyph)d).Update()));

    private readonly RowHover _row;
    private bool _shown;

    public RowPlayGlyph()
    {
        var resources = Application.Current.Resources;
        IsHitTestVisible = false;
        Opacity = 0;
        Background = (Brush)resources["JfScrim"];
        CornerRadius = new CornerRadius(TokenValues.Number("Artwork.RadiusList"));
        AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);
        Children.Add(new StrokeIcon
        {
            Data = Bind.Glyph("IconPlay"),
            Filled = true,
            Size = 18,
            Foreground = (Brush)resources["JfOnAccent"],
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        });
        _row = new RowHover(this, Update);
    }

    /// <summary>Choosing the row plays it here (a song that only opens its link shows no play button).</summary>
    public bool IsPlayable
    {
        get => (bool)GetValue(IsPlayableProperty);
        set => SetValue(IsPlayableProperty, value);
    }

    /// <summary>Fades in or out over dur-1 Fluid (at once with reduced motion) when the row's state changes.</summary>
    private void Update()
    {
        var shown = _row is not null && _row.IsActive && IsPlayable;
        if (shown == _shown)
        {
            return;
        }

        _shown = shown;
        Motion.Fade(this, shown ? 1 : 0, AppHost.App.ReduceMotion ? TimeSpan.Zero : TokenValues.Duration(1), TokenValues.Fluid);
    }
}
