using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace Jellyfesh.App.Shell;

/// <summary>
/// A sort menu on a button (docs/ui.md, Albums and Songs): the orders as radio items, a separator
/// and Descending. The view model keeps the choice; the button shows the current order's name and
/// the menu shows the choices each time it opens.
/// </summary>
internal sealed class SortMenu<T>
    where T : struct, Enum
{
    private readonly MenuFlyout _menu = new() { Placement = FlyoutPlacementMode.BottomEdgeAlignedRight };
    private readonly TextBlock _label;
    private readonly IReadOnlyList<T> _sorts;
    private readonly Func<T> _sort;
    private readonly Action<T> _setSort;
    private readonly Func<bool> _descending;
    private readonly Action<bool> _setDescending;

    public SortMenu(Button button, TextBlock label, IReadOnlyList<T> sorts, Func<T> sort, Action<T> setSort, Func<bool> descending, Action<bool> setDescending)
    {
        _label = label;
        _sorts = sorts;
        _sort = sort;
        _setSort = setSort;
        _descending = descending;
        _setDescending = setDescending;
        _menu.Opening += (_, _) => Build();
        button.Flyout = _menu;
        Refresh();
    }

    /// <summary>The button's label follows the view model's order.</summary>
    public void Refresh() => _label.Text = Name(_sort());

    private static string Name(T sort) => AppHost.L.Get("Sort_" + sort);

    private void Build()
    {
        _menu.Items.Clear();
        var current = _sort();
        foreach (var sort in _sorts)
        {
            var choice = sort;
            var item = new RadioMenuFlyoutItem { Text = Name(choice), GroupName = "Sort", IsChecked = EqualityComparer<T>.Default.Equals(choice, current) };
            item.Click += (_, _) =>
            {
                _setSort(choice);
                Refresh();
            };
            _menu.Items.Add(item);
        }

        _menu.Items.Add(new MenuFlyoutSeparator());
        var descending = new ToggleMenuFlyoutItem { Text = AppHost.L.Get("Sort_Descending"), IsChecked = _descending() };
        descending.Click += (_, _) => _setDescending(descending.IsChecked);
        _menu.Items.Add(descending);
    }
}
