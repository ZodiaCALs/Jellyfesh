using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Jellyfesh.App.Controls;

/// <summary>
/// Whether a list row is pointed at or has keyboard focus (docs/ui.md, "Rows and cards"): the row
/// is the list item (SelectorItem) that holds the owner. Lists recycle their items, so it finds the
/// item each time the owner loads and lets go when it unloads. The pointer counts while it is over
/// the item or anything in it; focus counts while the item, or something in it such as its More
/// button, has keyboard focus. A click that focuses the row does not keep it shown once the pointer
/// leaves.
/// </summary>
internal sealed class RowHover
{
    private readonly FrameworkElement _owner;
    private readonly Action _changed;
    private SelectorItem? _item;
    private bool _pointer;
    private bool _focus;

    public RowHover(FrameworkElement owner, Action changed)
    {
        _owner = owner;
        _changed = changed;
        owner.Loaded += (_, _) => Attach();
        owner.Unloaded += (_, _) =>
        {
            // An element moved within the tree may load again before its last Unloaded arrives.
            if (!_owner.IsLoaded)
            {
                Detach();
            }
        };
    }

    /// <summary>The list item holding the owner, while the owner is loaded in one.</summary>
    public SelectorItem? Item => _item;

    /// <summary>The row is pointed at or has keyboard focus.</summary>
    public bool IsActive => _pointer || _focus;

    private void Attach()
    {
        DependencyObject? node = VisualTreeHelper.GetParent(_owner);
        while (node is not null and not SelectorItem)
        {
            node = VisualTreeHelper.GetParent(node);
        }

        var item = node as SelectorItem;
        if (ReferenceEquals(item, _item))
        {
            return;
        }

        Detach();
        _item = item;
        if (item is null)
        {
            return;
        }

        item.PointerEntered += OnPointerEntered;
        item.PointerExited += OnPointerExited;
        item.PointerCanceled += OnPointerCanceled;
        item.GotFocus += OnFocusChanged;
        item.LostFocus += OnFocusChanged;
        _focus = HasKeyboardFocus(item);
        _changed();
    }

    private void Detach()
    {
        if (_item is { } item)
        {
            item.PointerEntered -= OnPointerEntered;
            item.PointerExited -= OnPointerExited;
            item.PointerCanceled -= OnPointerCanceled;
            item.GotFocus -= OnFocusChanged;
            item.LostFocus -= OnFocusChanged;
            _item = null;
        }

        if (_pointer || _focus)
        {
            _pointer = false;
            _focus = false;
            _changed();
        }
    }

    private void OnPointerEntered(object sender, PointerRoutedEventArgs e) => SetPointer(true);

    /// <summary>
    /// The pointer left the item, or left something in it: the item itself only once the pointer is
    /// outside it (pointer exits of what it holds reach it too).
    /// </summary>
    private void OnPointerExited(object sender, PointerRoutedEventArgs e)
    {
        if (_item is not { } item)
        {
            return;
        }

        if (ReferenceEquals(e.OriginalSource, item))
        {
            SetPointer(false);
            return;
        }

        var at = e.GetCurrentPoint(item).Position;
        SetPointer(at.X >= 0 && at.Y >= 0 && at.X < item.ActualWidth && at.Y < item.ActualHeight);
    }

    private void OnPointerCanceled(object sender, PointerRoutedEventArgs e) => SetPointer(false);

    private void OnFocusChanged(object sender, RoutedEventArgs e)
    {
        var focus = _item is { } item && HasKeyboardFocus(item);
        if (focus != _focus)
        {
            _focus = focus;
            _changed();
        }
    }

    private void SetPointer(bool over)
    {
        if (over != _pointer)
        {
            _pointer = over;
            _changed();
        }
    }

    /// <summary>The element with focus is the item or inside it, and has it from the keyboard (or from code).</summary>
    private static bool HasKeyboardFocus(SelectorItem item)
    {
        if (item.XamlRoot is not { } root
            || FocusManager.GetFocusedElement(root) is not Control { FocusState: FocusState.Keyboard or FocusState.Programmatic } focused)
        {
            return false;
        }

        for (DependencyObject? node = focused; node is not null; node = VisualTreeHelper.GetParent(node))
        {
            if (ReferenceEquals(node, item))
            {
                return true;
            }
        }

        return false;
    }
}
