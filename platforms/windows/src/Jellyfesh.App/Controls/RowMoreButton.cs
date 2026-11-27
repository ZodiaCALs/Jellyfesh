using Jellyfesh.App.Shell;
using Jellyfesh.App.Theming;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace Jellyfesh.App.Controls;

/// <summary>
/// A row's More button at its end (docs/ui.md, "Rows and cards"; the web's ItemRow): shown while
/// the row is pointed at or has keyboard focus, and while its menu is open. It opens the row's menu,
/// the same one right-click opens: the list item's context menu, which Shell.Lists gives every row
/// (the song menu, an album's or a playlist's, the queue's). Tab reaches it from its row; a row
/// without a menu shows none.
/// </summary>
public sealed partial class RowMoreButton : Button
{
    private readonly RowHover _row;
    private FlyoutBase? _open;
    private bool _shown;

    public RowMoreButton()
    {
        Style = (Style)Application.Current.Resources["JfIconButtonStyle"];
        Width = 40;
        Height = 40;
        VerticalAlignment = VerticalAlignment.Center;
        Opacity = 0;
        Content = new StrokeIcon { Data = Bind.Glyph("IconMore"), Size = 18 };
        var label = AppHost.L.Get("Row_More");
        AutomationProperties.SetName(this, label);
        ToolTipService.SetToolTip(this, label);
        _row = new RowHover(this, Update);
        Click += OnClick;
    }

    private void OnClick(object sender, RoutedEventArgs e)
    {
        if (_row.Item?.ContextFlyout is not { } menu)
        {
            return;
        }

        if (_open is null)
        {
            _open = menu;
            menu.Closed += OnMenuClosed;
        }

        Update();
        menu.ShowAt(this, new FlyoutShowOptions { Placement = FlyoutPlacementMode.BottomEdgeAlignedRight });
    }

    private void OnMenuClosed(object? sender, object e)
    {
        if (_open is { } menu)
        {
            menu.Closed -= OnMenuClosed;
            _open = null;
        }

        Update();
    }

    /// <summary>Fades in or out over dur-1 Fluid (at once with reduced motion) when the row's state changes.</summary>
    private void Update()
    {
        var shown = _row is not null && (_row.IsActive || _open is not null) && _row.Item?.ContextFlyout is not null;
        if (shown == _shown)
        {
            return;
        }

        _shown = shown;
        Motion.Fade(this, shown ? 1 : 0, AppHost.App.ReduceMotion ? TimeSpan.Zero : TokenValues.Duration(1), TokenValues.Fluid);
    }
}
