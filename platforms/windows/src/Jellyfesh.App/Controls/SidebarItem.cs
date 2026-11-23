using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Controls;

/// <summary>
/// A place in the sidebar panel, the phone tab bar or the Library tab's switcher (ADR-0023, after
/// Jellyfesh Web's nav items and tabs): a glyph and a label, chosen like a radio button, so one place
/// of a group is current. Its look is one of the JfSidebarItemStyle, JfPhoneTabStyle and
/// JfLibraryTabStyle styles (Styles/Controls.xaml), which draw the rest and current states as
/// separate layers, so hovering never fights the current place's colours.
/// </summary>
public sealed partial class SidebarItem : RadioButton
{
    public static readonly DependencyProperty LabelProperty =
        DependencyProperty.Register(nameof(Label), typeof(string), typeof(SidebarItem), new PropertyMetadata(string.Empty, (d, e) =>
            AutomationProperties.SetName(d, (string)e.NewValue)));

    public static readonly DependencyProperty GlyphProperty =
        DependencyProperty.Register(nameof(Glyph), typeof(string), typeof(SidebarItem), new PropertyMetadata(string.Empty));

    /// <summary>The place's name, also its accessible name.</summary>
    public string Label
    {
        get => (string)GetValue(LabelProperty);
        set => SetValue(LabelProperty, value);
    }

    /// <summary>The place's Misty glyph (path data from Styles/Controls.xaml).</summary>
    public string Glyph
    {
        get => (string)GetValue(GlyphProperty);
        set => SetValue(GlyphProperty, value);
    }

    /// <summary>Where choosing it goes: a place's tag ("albums", "library", "settings").</summary>
    /// <remarks>A settable property, not init: WinUI's generated type information sets the public properties of types XAML names.</remarks>
    public string Place { get; set; } = string.Empty;
}
