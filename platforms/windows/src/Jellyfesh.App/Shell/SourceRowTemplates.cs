using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Shell;

/// <summary>
/// The row template for a source page's mixed rows (Templates.xaml): an album, artist, playlist or
/// folder as an entry row, a song as a track row, numbered on an album.
/// </summary>
internal sealed partial class SourceRowTemplates(bool numbered) : DataTemplateSelector
{
    protected override DataTemplate SelectTemplateCore(object item) => Select(item);

    protected override DataTemplate SelectTemplateCore(object item, DependencyObject container) => Select(item);

    private DataTemplate Select(object item)
    {
        var key = item switch
        {
            SourceEntryItem => "SourceEntryTemplate",
            _ when numbered => "NumberedTrackRowTemplate",
            _ => "TrackRowTemplate",
        };
        return (DataTemplate)Application.Current.Resources[key];
    }
}
