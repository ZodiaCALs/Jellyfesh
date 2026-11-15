using Jellyfesh.App.Shell;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Views;

public sealed partial class SongsPage : JfPage
{
    public SongsPage()
    {
        ViewModel = AppHost.App.CreateSongs();
        PageModel = ViewModel;
        InitializeComponent();
        _ = new SortMenu<TrackSort>(SortButton, SortText, ViewModel.Sorts, () => ViewModel.Sort, s => ViewModel.Sort = s, () => ViewModel.Descending, d => ViewModel.Descending = d);
        Lists.Attach(SongList, 40, i => ViewModel.Songs?.Realize(i), new TrackMenu(AppHost.Services).Flyout);
    }

    public SongsViewModel ViewModel { get; }

    private void OnSongClick(object sender, ItemClickEventArgs e) => ViewModel.PlayFromCommand.Execute(e.ClickedItem as TrackItem);
}
