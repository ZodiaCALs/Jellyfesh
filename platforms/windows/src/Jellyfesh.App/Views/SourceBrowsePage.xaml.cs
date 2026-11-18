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
            PageModel = ViewModel;
            RowList.ItemTemplateSelector = new SourceRowTemplates(ViewModel.IsNumbered);
            Bindings.Update();
        }

        base.OnNavigatedTo(e);
    }

    private void OnRowClick(object sender, ItemClickEventArgs e) => ViewModel.OpenCommand.Execute(e.ClickedItem);

    private static void OnOpenRequested(SourceTarget target) => AppHost.Window.Navigate(typeof(SourceBrowsePage), target);
}
