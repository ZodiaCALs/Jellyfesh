using Jellyfesh.App.Shell;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Views;

public sealed partial class ArtistsPage : JfPage
{
    public ArtistsPage()
    {
        ViewModel = AppHost.App.CreateArtists();
        PageModel = ViewModel;
        InitializeComponent();
        var menu = new CollectionMenu(AppHost.Services, o => o is ArtistItem { IsLoaded: true } a ? new Collection.Artist(a.Id) : null);
        Lists.Attach(ArtistList, 0, i => ViewModel.Artists?.Realize(i), menu.Flyout);
    }

    public ArtistsViewModel ViewModel { get; }

    private void OnArtistClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is ArtistItem { Model: { } artist })
        {
            AppHost.Window.Navigate(typeof(ArtistPage), artist);
        }
    }
}
