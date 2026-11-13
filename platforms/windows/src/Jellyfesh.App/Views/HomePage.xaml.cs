using Jellyfesh.App.Shell;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Views;

/// <summary>Recently added, recently and most played, and playlists; an invitation to add music when empty.</summary>
public sealed partial class HomePage : JfPage
{
    public HomePage()
    {
        ViewModel = AppHost.App.CreateHome();
        PageModel = ViewModel;
        InitializeComponent();
        var menu = new TrackMenu(AppHost.Services).Flyout;
        foreach (var shelf in new[] { AddedShelf, PlayedShelf, MostShelf })
        {
            Lists.Attach(shelf, 148, menu: menu);
        }

        Lists.Attach(PlaylistShelf, 168, menu: new CollectionMenu(AppHost.Services, o => o is PlaylistItem p ? new Collection.Playlist(p.Id) : null).Flyout);
    }

    public HomeViewModel ViewModel { get; }

    private void OnTrackClick(object sender, ItemClickEventArgs e) => ViewModel.PlayCommand.Execute(e.ClickedItem as TrackItem);

    private void OnPlaylistClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is PlaylistItem item)
        {
            AppHost.Window.Navigate(typeof(PlaylistPage), item.Model);
        }
    }

    private async void OnAddFolder(object sender, RoutedEventArgs e)
    {
        var folder = await AppHost.Platform.PickFolderAsync();
        if (folder is not null)
        {
            AppHost.Services.Send(new CoreCommand.AddRoot(folder));
        }
    }
}
