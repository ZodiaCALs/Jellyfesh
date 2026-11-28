using Jellyfesh.App.Views;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Shell;

/// <summary>
/// The menu of an album, artist, playlist or folder from a source (search and browse rows, the
/// sources' sections, Discover's cards; docs/ui.md, "Rows and cards"), which right-click and More
/// open: Play, Shuffle, Play Next, Play Last and Add to Playlist… for an album or playlist that
/// plays here (a source that plays on the person's own devices is never queued, ADR-0018), Go to
/// Artist when the source names the artist, Download when its source allows downloads (ADR-0025),
/// and Open in {Service} when it has a link ("Links and remote playback"). What each acts on and
/// when it is offered is CardActions'. One menu serves a whole list; it reads its row when it opens.
/// </summary>
internal sealed class EntryMenu
{
    private readonly ShellServices _s;
    private readonly MenuFlyoutItem _play;
    private readonly MenuFlyoutItem _shuffle;
    private readonly MenuFlyoutItem _playNext;
    private readonly MenuFlyoutItem _playLast;
    private readonly MenuFlyoutSubItem _addTo;
    private readonly MenuFlyoutItem _goToArtist;
    private readonly MenuFlyoutItem _download;
    private readonly MenuFlyoutItem _open;
    private SourceEntryItem? _item;

    public EntryMenu(ShellServices services)
    {
        _s = services;
        var l = services.L;
        Flyout = new MenuFlyout();
        _play = Add(l.Get("Menu_Play"), c => CollectionActions.Play(_s, c));
        _shuffle = Add(l.Get("Menu_Shuffle"), c => CollectionActions.Play(_s, c, shuffle: true));
        _playNext = Add(l.Get("Menu_PlayNext"), c => CollectionActions.PlayNext(_s, c));
        _playLast = Add(l.Get("Menu_PlayLast"), c => CollectionActions.PlayLast(_s, c));
        _addTo = new MenuFlyoutSubItem { Text = l.Get("Menu_AddToPlaylist") };
        Flyout.Items.Add(_addTo);

        _goToArtist = new MenuFlyoutItem { Text = l.Get("Menu_GoToArtist") };
        _goToArtist.Click += (_, _) =>
        {
            if (CardActions.SourceArtist(_item) is { } artist)
            {
                AppHost.Window.Navigate(typeof(SourceBrowsePage), artist);
            }
        };
        Flyout.Items.Add(_goToArtist);

        // Download: a source's album or playlist, when its source allows downloads (ADR-0025).
        _download = new MenuFlyoutItem { Text = l.Get("Menu_Download") };
        _download.Click += (_, _) =>
        {
            if (DownloadActions.For(_s.Hub.State.Sources, _item) is { } target)
            {
                DownloadActions.Download(_s, target);
            }
        };
        Flyout.Items.Add(_download);

        _open = new MenuFlyoutItem();
        _open.Click += (_, _) => ItemLinks.Open(_s, _item?.Link);
        Flyout.Items.Add(_open);
        Flyout.Opening += OnOpening;
    }

    public MenuFlyout Flyout { get; }

    /// <summary>An entry that acts on what the row plays (CardActions.CollectionOf).</summary>
    private MenuFlyoutItem Add(string text, Action<Collection> action)
    {
        var item = new MenuFlyoutItem { Text = text };
        item.Click += (_, _) =>
        {
            if (CardActions.CollectionOf(_item) is { } collection)
            {
                action(collection);
            }
        };
        Flyout.Items.Add(item);
        return item;
    }

    private static Visibility Shown(bool shown) => shown ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>
    /// Fits the menu to its row: Play always (dimmed for an artist, a folder, or what does not play
    /// here), the rest of the playing entries only for what plays here.
    /// </summary>
    private async void OnOpening(object? sender, object e)
    {
        var item = _item = Lists.ItemOf<SourceEntryItem>(Flyout);
        var sources = _s.Hub.State.Sources;
        var plays = CardActions.CollectionOf(item) is not null;
        _play.IsEnabled = plays;
        _shuffle.Visibility = Shown(plays);
        var queues = CardActions.CanQueue(sources, item);
        _playNext.Visibility = Shown(queues);
        _playLast.Visibility = Shown(queues);
        _addTo.Visibility = Shown(queues);
        _goToArtist.Visibility = Shown(CardActions.SourceArtist(item) is not null);
        _download.Visibility = Shown(DownloadActions.For(sources, item) is not null);
        _open.Visibility = Shown(item is { HasLink: true });
        _open.Text = ItemLinks.OpenInLabel(_s.L, ItemLinks.ServiceName(sources, item?.Item.Source));

        // Add to Playlist…: the songs are read only when a playlist is chosen.
        var collection = CardActions.CanAddToPlaylist(sources, item) ? CardActions.CollectionOf(item) : null;
        _addTo.IsEnabled = collection is not null;
        if (collection is null)
        {
            return;
        }

        try
        {
            await PlaylistMenu.FillAsync(_addTo, _s, () => PlaylistActions.TracksOfAsync(_s, collection));
        }
        catch (Exception ex)
        {
            // async void: an exception would end the app. The submenu keeps what it had.
            AppLog.Write("filling Add to Playlist… failed", ex);
        }
    }
}
