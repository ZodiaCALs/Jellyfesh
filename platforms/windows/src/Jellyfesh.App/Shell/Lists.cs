using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace Jellyfesh.App.Shell;

/// <summary>
/// Virtualised lists of library rows (GridView, ListView): as the list realises a container it
/// pages the row in (placeholders fill when their page of the core's query arrives), asks for the
/// row's thumbnail at the size it is drawn, off the UI thread, and gives it the context menu; a
/// recycled container cancels its thumbnail. Nothing is loaded for rows that are never shown.
/// </summary>
internal static class Lists
{
    public static void Attach(ListViewBase list, double artworkSize, Action<int>? realize = null, FlyoutBase? menu = null)
    {
        list.ContainerContentChanging += (sender, args) =>
        {
            if (args.InRecycleQueue)
            {
                (args.Item as ArtworkItem)?.CancelArtwork();
                return;
            }

            if (args.Phase != 0)
            {
                return;
            }

            realize?.Invoke(args.ItemIndex);
            if (menu is not null && args.ItemContainer.ContextFlyout is null)
            {
                args.ItemContainer.ContextFlyout = menu;
            }

            if (args.Item is ArtworkItem item && artworkSize > 0)
            {
                var scale = sender.XamlRoot?.RasterizationScale ?? 1.0;
                item.RequestArtwork(AppHost.Services.Thumbnails, artworkSize * scale);
            }
        };
    }

    /// <summary>The row a context menu was opened on (its container's content or data context).</summary>
    public static T? ItemOf<T>(FlyoutBase flyout)
        where T : class =>
        flyout.Target switch
        {
            SelectorItem container when container.Content is T item => item,
            Microsoft.UI.Xaml.FrameworkElement element => element.DataContext as T,
            _ => null,
        };
}
