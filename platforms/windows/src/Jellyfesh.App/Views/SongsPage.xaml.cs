using Jellyfesh.App.Shell;
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
        SortBox.ItemsSource = ViewModel.Sorts.Select(s => AppHost.L.Get("Sort_" + s)).ToList();
        SortBox.SelectedIndex = ViewModel.Sorts.ToList().IndexOf(ViewModel.Sort);
        Lists.Attach(SongList, 40, i => ViewModel.Songs?.Realize(i), new TrackMenu(AppHost.Services).Flyout);
    }

    public SongsViewModel ViewModel { get; }

    private void OnSortChanged(object sender, SelectionChangedEventArgs e)
    {
        if (SortBox.SelectedIndex >= 0 && SortBox.SelectedIndex < ViewModel.Sorts.Count)
        {
            ViewModel.Sort = ViewModel.Sorts[SortBox.SelectedIndex];
        }
    }

    private void OnSongClick(object sender, ItemClickEventArgs e) => ViewModel.PlayFromCommand.Execute(e.ClickedItem as TrackItem);
}
