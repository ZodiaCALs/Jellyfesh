using Jellyfesh.App.Shell;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Views;

public sealed partial class FavoritesPage : JfPage
{
    public FavoritesPage()
    {
        ViewModel = AppHost.App.CreateFavorites();
        PageModel = ViewModel;
        InitializeComponent();
        var menu = new TrackMenu(AppHost.Services, "Menu_Unfavorite", item => ViewModel.UnfavoriteCommand.Execute(item));
        Lists.Attach(FavoriteList, 40, i => ViewModel.Favorites?.Realize(i), menu.Flyout);
        SourceLists.Model = ViewModel.Sources;
    }

    public FavoritesViewModel ViewModel { get; }

    private void OnTrackClick(object sender, ItemClickEventArgs e) => ViewModel.PlayTrackCommand.Execute(e.ClickedItem as TrackItem);
}
