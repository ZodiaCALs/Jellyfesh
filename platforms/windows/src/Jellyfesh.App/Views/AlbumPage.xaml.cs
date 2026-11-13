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

    private static void OnArtistRequested(Artist artist) => AppHost.Window.Navigate(typeof(ArtistPage), artist);
}
