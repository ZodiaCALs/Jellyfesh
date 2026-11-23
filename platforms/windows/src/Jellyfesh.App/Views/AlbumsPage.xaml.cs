using Jellyfesh.App.Controls;
using Jellyfesh.App.Shell;
using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace Jellyfesh.App.Views;

public sealed partial class AlbumsPage : JfPage
{
    public AlbumsPage()
    {
        ViewModel = AppHost.App.CreateAlbums();
        PageModel = ViewModel;
        InitializeComponent();
        _ = new SortMenu<AlbumSort>(SortButton, SortText, ViewModel.Sorts, () => ViewModel.Sort, s => ViewModel.Sort = s, () => ViewModel.Descending, d => ViewModel.Descending = d);
        var menu = new CollectionMenu(AppHost.Services, o => o is AlbumItem { IsLoaded: true } a ? new Collection.Album(a.Id) : null);
        Lists.Attach(AlbumGrid, () => _grid.Cover, i => ViewModel.Albums?.Realize(i), menu.Flyout);
        AlbumGrid.ContainerContentChanging += (_, args) =>
        {
            if (!args.InRecycleQueue)
            {
                Fit(args.ItemContainer);
            }
        };
    }

    public AlbumsViewModel ViewModel { get; }

    /// <summary>The grid's columns and covers for its width (Jellyfesh Web's measures).</summary>
    private CoverGrid _grid = CoverGrid.For(968, phone: false);

    /// <summary>
    /// Fits the covers to the width: the slots of the wrap grid, each container a cover wide with the
    /// spacing after it, and the grid stepping out on its trailing side by as much, so that the last
    /// column's spacing falls into the gutter and the first cover lines up with the title.
    /// </summary>
    private void OnGridSizeChanged(object sender, SizeChangedEventArgs e)
    {
        // The grid reaches the spacing beyond the content on its trailing side (its negative margin).
        var width = e.NewSize.Width + AlbumGrid.Margin.Right - AlbumGrid.Padding.Left - AlbumGrid.Padding.Right;
        var grid = CoverGrid.For(width, phone: !AppHost.Metrics.Layout.Sidebar);
        if (grid == _grid && AlbumGrid.Margin.Right == -grid.Spacing)
        {
            return;
        }

        _grid = grid;
        AlbumGrid.Margin = new Thickness(0, 0, -grid.Spacing, 0);
        HeaderRow.Margin = new Thickness(0, 0, grid.Spacing, 16);
        if (AlbumGrid.ItemsPanelRoot is ItemsWrapGrid wrap)
        {
            wrap.ItemWidth = grid.Slot;
            wrap.ItemHeight = grid.Cover + 72;
            foreach (var container in wrap.Children.OfType<GridViewItem>())
            {
                Fit(container);
            }
        }
    }

    private void Fit(SelectorItem container)
    {
        container.Margin = new Thickness(0, 0, _grid.Spacing, 16);
        if (container.ContentTemplateRoot is AlbumTile tile)
        {
            tile.CoverSize = _grid.Cover;
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
