using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Shell;

/// <summary>Discover's tiles (Templates.xaml): an album, artist or playlist, or a song.</summary>
public sealed partial class SourceTileTemplates : DataTemplateSelector
{
    protected override DataTemplate SelectTemplateCore(object item) => Select(item);

    protected override DataTemplate SelectTemplateCore(object item, DependencyObject container) => Select(item);

    private static DataTemplate Select(object item) =>
        (DataTemplate)Application.Current.Resources[item is SourceEntryItem ? "SourceTileTemplate" : "TrackTileTemplate"];
}
