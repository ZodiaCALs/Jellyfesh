using Jellyfesh.App.Shell;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Navigation;
using Windows.ApplicationModel.DataTransfer;

namespace Jellyfesh.App.Views;

public sealed partial class PlaylistPage : JfPage
{
    public PlaylistPage()
    {
        InitializeComponent();
        var menu = new TrackMenu(AppHost.Services, "Menu_RemoveFromPlaylist", item => ViewModel.RemoveEntryCommand.Execute(item));
        Lists.Attach(EntryList, 40, i => ViewModel.Entries?.Realize(i), menu.Flyout);
    }

    public PlaylistDetailViewModel ViewModel { get; private set; } = null!;

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        if (e.Parameter is PlaylistSummary playlist && ViewModel?.Playlist.Id != playlist.Id)
        {
            if (ViewModel is not null)
            {
                ViewModel.Deleted -= OnDeleted;
            }

            ViewModel = AppHost.App.CreatePlaylist(playlist);
            ViewModel.Deleted += OnDeleted;
            PageModel = ViewModel;
            Bindings.Update();
        }

        base.OnNavigatedTo(e);
    }

    private void OnEntryClick(object sender, ItemClickEventArgs e) => ViewModel.PlayTrackCommand.Execute(e.ClickedItem as TrackItem);

    /// <summary>More › Add to Playlist…: New Playlist… and the playlists, with this playlist's songs.</summary>
    private async void OnMoreOpening(object? sender, object e) => await PlaylistMenu.FillAsync(AddToPlaylistItem, AppHost.Services, ViewModel.TracksAsync);

    /// <summary>Delete Playlist… removed this playlist: back to where the person came from.</summary>
    private void OnDeleted()
    {
        if (Frame?.CanGoBack == true)
        {
            Frame.GoBack();
        }
        else
        {
            AppHost.Window.Navigate(typeof(PlaylistsPage));
        }
    }

    private void OnDragItemsCompleted(ListViewBase sender, DragItemsCompletedEventArgs args)
    {
        if (args.DropResult == DataPackageOperation.Move && args.Items.FirstOrDefault() is TrackItem item && ViewModel.Entries is { } entries)
        {
            ViewModel.MoveEntry(item, entries.Items.IndexOf(item));
        }
    }

    private void OnMoveUp(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Move(args, -1);

    private void OnMoveDown(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Move(args, 1);

    private void Move(KeyboardAcceleratorInvokedEventArgs args, int by)
    {
        if (EntryList.SelectedItem is TrackItem item && ViewModel.Entries is { } entries)
        {
            var to = entries.Items.IndexOf(item) + by;
            if (to >= 0 && to < entries.Items.Count)
            {
                ViewModel.MoveEntry(item, to);
            }

            args.Handled = true;
        }
    }

    private void OnRemoveKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (EntryList.SelectedItem is TrackItem item)
        {
            ViewModel.RemoveEntryCommand.Execute(item);
            args.Handled = true;
        }
    }
}
