using Jellyfesh.App.Shell;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Views;

public sealed partial class SourcesPage : JfPage
{
    public SourcesPage()
    {
        ViewModel = AppHost.App.CreateSources();
        PageModel = ViewModel;
        InitializeComponent();
        Lists.Attach(EntryList, 40);
        Lists.Attach(TrackList, 40, menu: new TrackMenu(AppHost.Services).Flyout);
        ViewModel.SignInRequested += SourceDialogs.ShowSignIn;
    }

    public SourcesViewModel ViewModel { get; }

    /// <summary>Back (Esc, the title bar, the mouse): up a level of the browsed source, then to the list of sources.</summary>
    public bool GoUp() => ViewModel.GoUp();

    /// <summary>A service in Add a Source: "Connect to …", then the sign-in dialog.</summary>
    private async void OnPresetClick(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is SourcePresetViewModel preset)
        {
            await SourceDialogs.ConnectAsync(preset, ViewModel.Connect);
        }
    }

    /// <summary>A source in Your Sources: browse it.</summary>
    private void OnSourceClick(object sender, RoutedEventArgs e) => ViewModel.OpenCommand.Execute((sender as FrameworkElement)?.Tag as SourceChoice);

    private void OnCrumbClicked(BreadcrumbBar sender, BreadcrumbBarItemClickedEventArgs args) =>
        ViewModel.GoToCommand.Execute(args.Item as BrowseLevel);

    private void OnEntryClick(object sender, ItemClickEventArgs e) => ViewModel.OpenEntryCommand.Execute(e.ClickedItem as SourceEntryItem);

    private void OnTrackClick(object sender, ItemClickEventArgs e) => ViewModel.PlayTrackCommand.Execute(e.ClickedItem as TrackItem);

    /// <summary>Sources Settings… opens Settings › Sources.</summary>
    private void OnManageClick(object sender, RoutedEventArgs e) => AppHost.Window.Navigate(typeof(SettingsPage), SettingsSection.Sources);
}
