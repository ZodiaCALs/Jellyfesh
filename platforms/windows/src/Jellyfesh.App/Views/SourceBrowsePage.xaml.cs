using Jellyfesh.App.Shell;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Jellyfesh.App.Views;

/// <summary>A page of a source's catalogue; its view model decides what a row does (SourceBrowseViewModel).</summary>
public sealed partial class SourceBrowsePage : JfPage
{
    public SourceBrowsePage()
    {
        InitializeComponent();
        var entryMenu = new EntryMenu(AppHost.Services).Flyout;
        var trackMenu = new TrackMenu(AppHost.Services).Flyout;
        Lists.Attach(RowList, 40, null, row => row switch
        {
            SourceEntryItem => entryMenu,
            TrackItem => trackMenu,
            _ => null,
        });
    }

    public SourceBrowseViewModel ViewModel { get; private set; } = null!;

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        if (e.Parameter is SourceTarget target && ViewModel?.Target != target)
        {
            if (ViewModel is not null)
            {
                ViewModel.OpenRequested -= OnOpenRequested;
            }

            ViewModel = AppHost.App.CreateSourceBrowse(target);
            ViewModel.OpenRequested += OnOpenRequested;
            ViewModel.HeaderChanged += () => ViewModel.Header?.RequestArtwork(AppHost.Services.Thumbnails, 240 * (XamlRoot?.RasterizationScale ?? 1.5));
            PageModel = ViewModel;
            RowList.ItemTemplateSelector = new SourceRowTemplates(ViewModel.IsNumbered);
            Bindings.Update();

            // The header's cover comes through the core, at the size it is drawn.
            ViewModel.Header?.RequestArtwork(AppHost.Services.Thumbnails, 240 * (XamlRoot?.RasterizationScale ?? 1.5));
        }

        base.OnNavigatedTo(e);
    }

    private void OnRowClick(object sender, ItemClickEventArgs e) => ViewModel.OpenCommand.Execute(e.ClickedItem);

    /// <summary>More: the notes or biography whole.</summary>
    private void OnDescriptionMoreClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        DescriptionText.MaxLines = 0;
        DescriptionMore.Visibility = Microsoft.UI.Xaml.Visibility.Collapsed;
    }

    /// <summary>Get Info for the album or artist this page shows (its source's details).</summary>
    private void OnGetInfoClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        if (ViewModel.DetailsItem is { } item)
        {
            InfoDialog.ShowFor(AppHost.Services, item, ViewModel.Target.Kind == Core.ItemKind.Artist ? InfoKind.Artist : InfoKind.Album, ViewModel.Title);
        }
    }

    private static void OnOpenRequested(SourceTarget target) => AppHost.Window.Navigate(typeof(SourceBrowsePage), target);
}
