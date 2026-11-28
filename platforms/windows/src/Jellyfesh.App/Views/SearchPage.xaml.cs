using Jellyfesh.App.Shell;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Windows.System;

namespace Jellyfesh.App.Views;

public sealed partial class SearchPage : JfPage
{
    /// <summary>The song menu, shared by the library's songs and every source's.</summary>
    private readonly FlyoutBase _menu;

    /// <summary>The menu of the sources' albums, artists, playlists and folders (Play, Open in {Service}).</summary>
    private readonly FlyoutBase _entryMenu;

    public SearchPage()
    {
        ViewModel = AppHost.App.CreateSearch();
        PageModel = ViewModel;
        InitializeComponent();
        _menu = new TrackMenu(AppHost.Services).Flyout;
        _entryMenu = new EntryMenu(AppHost.Services).Flyout;
        Lists.Attach(TrackList, 40, menu: _menu);
        Lists.Attach(AlbumList, 40, menu: new CollectionMenu(AppHost.Services, o => o is AlbumItem a ? new Collection.Album(a.Id) : null, goToArtist: true).Flyout);
        Lists.Attach(ArtistList, 0, menu: new CollectionMenu(AppHost.Services, o => o is ArtistItem { IsLoaded: true } a ? new Collection.Artist(a.Id) : null).Flyout);
        ViewModel.OpenRequested += target => AppHost.Window.Navigate(typeof(SourceBrowsePage), target);
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

    /// <summary>A tab: All, Library or a source's.</summary>
    private void OnTabClick(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.DataContext is SearchTabItem tab)
        {
            ViewModel.SelectTabCommand.Execute(tab.Id);
        }
    }

    /// <summary>"Results from {Service}": that source's tab.</summary>
    private void OnApartClick(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.DataContext is SourceResultGroup group)
        {
            ViewModel.SelectTabCommand.Execute(group.Source);
        }
    }

    /// <summary>A source's group of songs (From Your Sources): thumbnails and the song menu, once per list.</summary>
    private void OnOnlineTracksLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is ListView { Tag: null } list)
        {
            list.Tag = this;
            Lists.Attach(list, 40, menu: _menu);
        }
    }

    /// <summary>A source's group of albums, artists, playlists and folders: thumbnails and their menu, once per list.</summary>
    private void OnOnlineEntriesLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is ListView { Tag: null } list)
        {
            list.Tag = this;
            Lists.Attach(list, 40, menu: _entryMenu);
        }
    }

    private void OnOnlineTrackClick(object sender, ItemClickEventArgs e) => ViewModel.PlayTrackCommand.Execute(e.ClickedItem as TrackItem);

    private void OnEntryClick(object sender, ItemClickEventArgs e) => ViewModel.OpenEntryCommand.Execute(e.ClickedItem as SourceEntryItem);

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
