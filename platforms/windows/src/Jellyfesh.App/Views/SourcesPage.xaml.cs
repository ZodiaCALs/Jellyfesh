using Jellyfesh.App.Shell;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;

namespace Jellyfesh.App.Views;

public sealed partial class SourcesPage : JfPage
{
    public SourcesPage()
    {
        ViewModel = AppHost.App.CreateSources();
        PageModel = ViewModel;
        InitializeComponent();
        ViewModel.SignInRequested += SourceDialogs.ShowSignIn;
        // Choosing a source opens its Discover page (Browse there opens its folders).
        ViewModel.BrowseRequested += target => AppHost.Window.Navigate(typeof(SourceDiscoverPage), target);
    }

    public SourcesViewModel ViewModel { get; }

    /// <summary>A service in Add a Source: "Connect to …", then the sign-in dialog.</summary>
    private async void OnPresetClick(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is SourcePresetViewModel preset)
        {
            await SourceDialogs.ConnectAsync(preset, ViewModel.Connect);
        }
    }

    /// <summary>A source in Your Sources: open its catalogue.</summary>
    private void OnSourceClick(object sender, RoutedEventArgs e) => ViewModel.OpenCommand.Execute((sender as FrameworkElement)?.Tag as SourceChoice);

    /// <summary>Sources Settings… opens Settings › Sources.</summary>
    private void OnManageClick(object sender, RoutedEventArgs e) => AppHost.Window.Navigate(typeof(SettingsPage), SettingsSection.Sources);
}
