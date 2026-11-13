using Jellyfesh.App.Shell;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Jellyfesh.App.Views;

public sealed partial class ArtistPage : JfPage
{
    public ArtistPage()
    {
        InitializeComponent();
        Lists.Attach(SongList, 40, menu: new TrackMenu(AppHost.Services).Flyout);
        Lists.Attach(AlbumShelf, 168, menu: new CollectionMenu(AppHost.Services, o => o is AlbumItem a ? new Collection.Album(a.Id) : null).Flyout);
    }

    public ArtistDetailViewModel ViewModel { get; private set; } = null!;

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        if (e.Parameter is Artist artist && ViewModel?.Artist.Id != artist.Id)
        {
            ViewModel = AppHost.App.CreateArtist(artist);
            PageModel = ViewModel;
            Monogram.Text = artist.Name.Length > 0 ? char.ToUpperInvariant(artist.Name[0]).ToString() : string.Empty;
            Bindings.Update();
        }

        base.OnNavigatedTo(e);
    }

    private void OnSongClick(object sender, ItemClickEventArgs e) => ViewModel.PlayTrackCommand.Execute(e.ClickedItem as TrackItem);

    private void OnAlbumClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is AlbumItem { Model: { } album })
        {
            AppHost.Window.Navigate(typeof(AlbumPage), album);
        }
    }
}
