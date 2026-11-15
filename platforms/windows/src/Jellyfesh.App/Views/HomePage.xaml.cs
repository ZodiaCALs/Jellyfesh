using Jellyfesh.App.Shell;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Views;

/// <summary>The library's size, Recently Added albums, Recently Played and Most Played songs; the welcome when there is nothing yet.</summary>
public sealed partial class HomePage : JfPage
{
    public HomePage()
    {
        ViewModel = AppHost.App.CreateHome();
        PageModel = ViewModel;
        InitializeComponent();
        Lists.Attach(AddedShelf, 168, menu: new CollectionMenu(AppHost.Services, o => o is AlbumItem a ? new Collection.Album(a.Id) : null).Flyout);
        var songs = new TrackMenu(AppHost.Services).Flyout;
        foreach (var shelf in new[] { PlayedShelf, MostShelf })
        {
            Lists.Attach(shelf, 148, menu: songs);
        }
    }

    public HomeViewModel ViewModel { get; }

    private void OnTrackClick(object sender, ItemClickEventArgs e) => ViewModel.PlayCommand.Execute(e.ClickedItem as TrackItem);

    private void OnAlbumClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is AlbumItem { Model: { } album })
        {
            AppHost.Window.Navigate(typeof(AlbumPage), album);
        }
    }

    /// <summary>Add a Source: the Sources page, which starts with Add a Source.</summary>
    private void OnAddSource(object sender, RoutedEventArgs e) => AppHost.Window.Navigate(typeof(SourcesPage));
}
