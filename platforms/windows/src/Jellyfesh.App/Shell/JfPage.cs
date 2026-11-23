using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Jellyfesh.App.Shell;

/// <summary>
/// A page whose view model listens to the core only while the page is shown (PageViewModel's
/// Activate and Deactivate), so hidden pages cost nothing; a library change while hidden reloads
/// on return.
/// </summary>
public partial class JfPage : Page
{
    /// <summary>The page's view model, set by the derived page before it is shown.</summary>
    protected PageViewModel? PageModel { get; set; }

    /// <summary>The layout's measures (gutters, the room at the end, the title size) for the page's bindings.</summary>
    public ShellMetrics Layout => AppHost.Metrics;

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        PageModel?.Activate();
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        PageModel?.Deactivate();
        base.OnNavigatedFrom(e);
    }
}
