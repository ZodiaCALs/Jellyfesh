using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Shell;

/// <summary>
/// The context menu of an album, artist, playlist or folder from a source (search and browse
/// rows): Play (when it plays here), and Open in {Service} when it has a link (docs/ui.md, "Links
/// and remote playback"). One menu serves a whole list; it reads its row when it opens.
/// </summary>
internal sealed class EntryMenu
{
    private readonly ShellServices _s;
    private readonly MenuFlyoutItem _play;
    private readonly MenuFlyoutItem _open;
    private readonly MenuFlyoutItem _download;
    private SourceEntryItem? _item;

    public EntryMenu(ShellServices services)
    {
        _s = services;
        Flyout = new MenuFlyout();
        _play = new MenuFlyoutItem { Text = services.L.Get("Menu_Play") };
        _play.Click += (_, _) =>
        {
            if (_item is { CanPlay: true } item)
            {
                CollectionActions.Play(_s, new Collection.Source(item.Item.Source, item.Item.Id, item.Item.Kind));
            }
        };
        _open = new MenuFlyoutItem();
        _open.Click += (_, _) => ItemLinks.Open(_s, _item?.Link);

        // Download: a source's album or playlist, when its source allows downloads (ADR-0025).
        _download = new MenuFlyoutItem { Text = services.L.Get("Menu_Download") };
        _download.Click += (_, _) =>
        {
            if (DownloadActions.For(_s.Hub.State.Sources, _item) is { } target)
            {
                DownloadActions.Download(_s, target);
            }
        };
        Flyout.Items.Add(_play);
        Flyout.Items.Add(_download);
        Flyout.Items.Add(_open);
        Flyout.Opening += (_, _) =>
        {
            _item = Lists.ItemOf<SourceEntryItem>(Flyout);
            _play.IsEnabled = _item is { CanPlay: true };
            _open.Visibility = _item is { HasLink: true } ? Visibility.Visible : Visibility.Collapsed;
            _download.Visibility = DownloadActions.For(_s.Hub.State.Sources, _item) is not null ? Visibility.Visible : Visibility.Collapsed;
            _open.Text = ItemLinks.OpenInLabel(_s.L, ItemLinks.ServiceName(_s.Hub.State.Sources, _item?.Item.Source));
        };
    }

    public MenuFlyout Flyout { get; }
}
