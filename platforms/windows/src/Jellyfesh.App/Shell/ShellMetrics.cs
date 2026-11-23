using System.ComponentModel;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;

namespace Jellyfesh.App.Shell;

/// <summary>
/// The layout's measures for pages to bind to (<see cref="ShellLayout"/> in WinUI's types): the
/// content's gutters and top, the room below a page's last row, so that it scrolls clear of the
/// floating player and phone tabs, and the page titles' size. One per window; the window changes it
/// with its width and with whether something plays.
/// </summary>
public sealed partial class ShellMetrics : INotifyPropertyChanged
{
    private ShellLayout _layout = ShellLayout.Initial;

    public event PropertyChangedEventHandler? PropertyChanged;

    /// <summary>The layout these measures come from.</summary>
    public ShellLayout Layout => _layout;

    /// <summary>A page's padding: the gutters and the top; its list or scroller pads its own end.</summary>
    public Thickness PagePadding => new(_layout.Gutter, _layout.Top, _layout.Gutter, 0);

    /// <summary>The end of a page's list or scrolled content: room for the floating capsules and the fade above them.</summary>
    public Thickness EndPadding => new(0, 0, 0, _layout.BottomPadding);

    /// <summary>A scrolled page's padding all round: the gutters, the top and the room at its end.</summary>
    public Thickness ScrollPadding => new(_layout.Gutter, _layout.Top, _layout.Gutter, _layout.BottomPadding);

    /// <summary>Page titles: 48 epx from 900 epx wide, 30.4 below (type.text-h1, text-h2).</summary>
    public double TitleSize => _layout.TitleSize;

    internal void Set(ShellLayout layout)
    {
        if (layout == _layout)
        {
            return;
        }

        _layout = layout;
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(string.Empty)); // every measure
    }
}
