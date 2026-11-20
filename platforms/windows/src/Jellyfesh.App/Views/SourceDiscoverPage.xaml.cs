using Jellyfesh.App.Shell;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Navigation;

namespace Jellyfesh.App.Views;

/// <summary>A source's Discover page; its view model asks the core for the shelves and decides what an item does.</summary>
public sealed partial class SourceDiscoverPage : JfPage
{
    private readonly HashSet<ListViewBase> _attached = [];
    private readonly FlyoutBase _entryMenu = new EntryMenu(AppHost.Services).Flyout;
    private readonly FlyoutBase _trackMenu = new TrackMenu(AppHost.Services).Flyout;

    public SourceDiscoverPage()
    {
        InitializeComponent();
    }

    public DiscoverViewModel ViewModel { get; private set; } = null!;

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        if (e.Parameter is SourceTarget target && ViewModel?.Source != target.Plugin)
        {
            if (ViewModel is not null)
            {
                ViewModel.OpenRequested -= OnOpenRequested;
            }

            ViewModel = AppHost.App.CreateDiscover(target.Plugin, target.Title);
            ViewModel.OpenRequested += OnOpenRequested;
            PageModel = ViewModel;
            Bindings.Update();
        }

        base.OnNavigatedTo(e);
    }

    /// <summary>Each shelf's list loads its covers through the core and gets the song and entry menus.</summary>
    private void OnShelfListLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is ListViewBase list && _attached.Add(list))
        {
            Lists.Attach(list, list is ListView ? 40 : 150, null, row => row switch
            {
                SourceEntryItem => _entryMenu,
                TrackItem => _trackMenu,
                _ => null,
            });
        }
    }

    private void OnShelfItemClick(object sender, ItemClickEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is ShelfViewModel shelf)
        {
            ViewModel.Open(shelf, e.ClickedItem);
        }
    }

    private void OnSeeAllClick(object sender, RoutedEventArgs e) =>
        ViewModel.SeeAllCommand.Execute((sender as FrameworkElement)?.Tag as ShelfViewModel);

    private static void OnOpenRequested(SourceTarget target) => AppHost.Window.Navigate(typeof(SourceBrowsePage), target);
}
