using System.ComponentModel;
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
        ViewModel.PropertyChanged += OnViewModelChanged;
    }

    public SourcesViewModel ViewModel { get; }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(SourcesViewModel.Selected) && !ReferenceEquals(SourceBox.SelectedItem, ViewModel.Selected))
        {
            SourceBox.SelectedItem = ViewModel.Selected;
        }
    }

    private void OnSourceChanged(object sender, SelectionChangedEventArgs e)
    {
        if (SourceBox.SelectedItem is SourceChoice choice && !ReferenceEquals(choice, ViewModel.Selected))
        {
            ViewModel.Selected = choice;
        }
    }

    private void OnCrumbClicked(BreadcrumbBar sender, BreadcrumbBarItemClickedEventArgs args) =>
        ViewModel.GoToCommand.Execute(args.Item as BrowseLevel);

    private void OnEntryClick(object sender, ItemClickEventArgs e) => ViewModel.OpenCommand.Execute(e.ClickedItem as SourceEntryItem);

    private void OnTrackClick(object sender, ItemClickEventArgs e) => ViewModel.PlayTrackCommand.Execute(e.ClickedItem as TrackItem);

    private void OnManageClick(object sender, RoutedEventArgs e) => AppHost.Window.Navigate(typeof(SettingsPage), SettingsSection.Sources);
}
