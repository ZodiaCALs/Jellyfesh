using Jellyfesh.App.Shell;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Views;

public sealed partial class AlbumsPage : JfPage
{
    public AlbumsPage()
    {
        ViewModel = AppHost.App.CreateAlbums();
        PageModel = ViewModel;
        InitializeComponent();
        SortBox.ItemsSource = ViewModel.Sorts.Select(s => AppHost.L.Get("Sort_" + s)).ToList();
        SortBox.SelectedIndex = ViewModel.Sorts.ToList().IndexOf(ViewModel.Sort);
        var menu = new CollectionMenu(AppHost.Services, o => o is AlbumItem { IsLoaded: true } a ? new Collection.Album(a.Id) : null);
        Lists.Attach(AlbumGrid, 168, i => ViewModel.Albums?.Realize(i), menu.Flyout);
    }

    public AlbumsViewModel ViewModel { get; }

    private void OnSortChanged(object sender, SelectionChangedEventArgs e)
    {
        if (SortBox.SelectedIndex >= 0 && SortBox.SelectedIndex < ViewModel.Sorts.Count)
        {
            ViewModel.Sort = ViewModel.Sorts[SortBox.SelectedIndex];
        }
    }

    private void OnAlbumClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is AlbumItem { Model: { } album })
        {
            AppHost.Window.Navigate(typeof(AlbumPage), album);
        }
    }
}
