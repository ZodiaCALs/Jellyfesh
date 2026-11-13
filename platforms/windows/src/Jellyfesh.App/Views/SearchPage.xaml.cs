using Jellyfesh.App.Shell;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.System;

namespace Jellyfesh.App.Views;

public sealed partial class SearchPage : JfPage
{
    public SearchPage()
    {
        ViewModel = AppHost.App.CreateSearch();
        PageModel = ViewModel;
        InitializeComponent();
        var menu = new TrackMenu(AppHost.Services).Flyout;
        Lists.Attach(TrackList, 40, menu: menu);
        Lists.Attach(OnlineTrackList, 40, menu: menu);
        Lists.Attach(OnlineEntryList, 40);
        Lists.Attach(AlbumShelf, 168);
        Lists.Attach(ArtistShelf, 0);
        Loaded += (_, _) => FocusBox();
    }

    public SearchViewModel ViewModel { get; }

    public void FocusBox()
    {
        Box.Focus(FocusState.Programmatic);
        Box.SelectAll();
    }

    private void OnBoxKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == VirtualKey.Enter)
        {
            ViewModel.SubmitCommand.Execute(null);
            e.Handled = true;
        }
    }

    private void OnTrackClick(object sender, ItemClickEventArgs e) => ViewModel.PlayTrackCommand.Execute(e.ClickedItem as TrackItem);

    private void OnOnlineTrackClick(object sender, ItemClickEventArgs e) => ViewModel.PlayTrackCommand.Execute(e.ClickedItem as TrackItem);

    private void OnEntryClick(object sender, ItemClickEventArgs e) => ViewModel.PlayEntryCommand.Execute(e.ClickedItem as SourceEntryItem);

    private void OnAlbumClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is AlbumItem { Model: { } album })
        {
            AppHost.Window.Navigate(typeof(AlbumPage), album);
        }
    }

    private void OnArtistClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is ArtistItem { Model: { } artist })
        {
            AppHost.Window.Navigate(typeof(ArtistPage), artist);
        }
    }
}
