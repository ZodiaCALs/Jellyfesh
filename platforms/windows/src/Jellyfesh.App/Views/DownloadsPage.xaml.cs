using Jellyfesh.App.Shell;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Views;

public sealed partial class DownloadsPage : JfPage
{
    public DownloadsPage()
    {
        ViewModel = AppHost.App.CreateDownloads();
        PageModel = ViewModel;
        InitializeComponent();
    }

    public DownloadsViewModel ViewModel { get; }

    private static T? ItemOf<T>(object sender)
        where T : class => (sender as FrameworkElement)?.DataContext as T;

    private void OnRetryClick(object sender, RoutedEventArgs e) => ViewModel.RetryCommand.Execute(ItemOf<DownloadRow>(sender));

    private void OnCancelClick(object sender, RoutedEventArgs e) => ViewModel.CancelCommand.Execute(ItemOf<DownloadRow>(sender));

    private void OnRemoveAlbumClick(object sender, RoutedEventArgs e) => ViewModel.RemoveAlbumCommand.Execute(ItemOf<DownloadedAlbum>(sender));

    private void OnRemoveSongClick(object sender, RoutedEventArgs e) => ViewModel.RemoveSongCommand.Execute(ItemOf<DownloadedSong>(sender));

    private void OnSongClick(object sender, ItemClickEventArgs e) => ViewModel.PlaySongCommand.Execute(e.ClickedItem as DownloadedSong);
}
