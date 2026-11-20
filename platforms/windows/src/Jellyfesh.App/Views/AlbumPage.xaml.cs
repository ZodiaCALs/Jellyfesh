using Jellyfesh.App.Shell;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Jellyfesh.App.Views;

public sealed partial class AlbumPage : JfPage
{
    public AlbumPage()
    {
        InitializeComponent();
        Lists.Attach(TrackList, 0, menu: new TrackMenu(AppHost.Services).Flyout);
    }

    public AlbumDetailViewModel ViewModel { get; private set; } = null!;

    /// <summary>Get Info: the album's facts and notes from the core.</summary>
    private void OnGetInfoClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e) =>
        InfoDialog.ShowFor(AppHost.Services, new Core.ItemRef.Album(ViewModel.Album.Id), InfoKind.Album, ViewModel.Album.Title);

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        if (e.Parameter is Album album && ViewModel?.Album.Id != album.Id)
        {
            if (ViewModel is not null)
            {
                ViewModel.ArtistRequested -= OnArtistRequested;
            }

            ViewModel = AppHost.App.CreateAlbum(album);
            ViewModel.ArtistRequested += OnArtistRequested;
            PageModel = ViewModel;
            ViewModel.Header.RequestArtwork(AppHost.Services.Thumbnails, 200 * (XamlRoot?.RasterizationScale ?? 1.5));
            Bindings.Update();
        }

        base.OnNavigatedTo(e);
    }

    private void OnTrackClick(object sender, ItemClickEventArgs e) => ViewModel.PlayTrackCommand.Execute(e.ClickedItem as TrackItem);

    /// <summary>More › Add to Playlist…: New Playlist… and the playlists, with the album's songs.</summary>
    private async void OnMoreOpening(object? sender, object e) => await PlaylistMenu.FillAsync(AddToPlaylistItem, AppHost.Services, ViewModel.TracksAsync);

    private static void OnArtistRequested(Artist artist) => AppHost.Window.Navigate(typeof(ArtistPage), artist);
}
