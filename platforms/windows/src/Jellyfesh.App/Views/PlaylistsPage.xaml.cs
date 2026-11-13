using Jellyfesh.App.Shell;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Views;

public sealed partial class PlaylistsPage : JfPage
{
    public PlaylistsPage()
    {
        ViewModel = AppHost.App.CreatePlaylists();
        PageModel = ViewModel;
        InitializeComponent();
        var menu = new CollectionMenu(
            AppHost.Services,
            o => o is PlaylistItem p ? new Collection.Playlist(p.Id) : null,
            "Menu_DeletePlaylist",
            o => ViewModel.DeleteCommand.Execute(o as PlaylistItem));
        Lists.Attach(PlaylistGrid, 168, menu: menu.Flyout);
    }

    public PlaylistsViewModel ViewModel { get; }

    private void OnPlaylistClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is PlaylistItem item)
        {
            AppHost.Window.Navigate(typeof(PlaylistPage), item.Model);
        }
    }
}
