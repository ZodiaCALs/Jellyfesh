using Microsoft.UI.Xaml;

namespace Jellyfesh.App.Styles;

/// <summary>The shared row and tile templates (a class, so its templates can use x:Bind).</summary>
public sealed partial class Templates : ResourceDictionary
{
    public Templates()
    {
        InitializeComponent();
    }
}
