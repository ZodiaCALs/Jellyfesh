using Jellyfesh.App.Views;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Shell;

/// <summary>
/// The context menu of an album, artist or playlist tile or row, which right-click and More open:
/// Play, Shuffle, Play Next, Play Last, Add to Playlist… (docs/parity.md N3), Go to Artist for a
/// library album where asked (not on its artist's own page; docs/ui.md, "Rows and cards"), and an
/// optional extra action (Delete Playlist…). One menu serves a whole grid or list.
/// </summary>
internal sealed class CollectionMenu
{
    private readonly ShellServices _s;
    private readonly Func<object, Collection?> _collectionOf;
    private readonly MenuFlyoutItem? _goToArtist;
    private object? _item;

    public CollectionMenu(ShellServices services, Func<object, Collection?> collectionOf, string? extraKey = null, Action<object>? extra = null, bool goToArtist = false)
    {
        _s = services;
        _collectionOf = collectionOf;
        var l = services.L;
        Flyout = new MenuFlyout();
        Add(l.Get("Menu_Play"), c => CollectionActions.Play(_s, c));
        Add(l.Get("Menu_Shuffle"), c => CollectionActions.Play(_s, c, shuffle: true));
        Add(l.Get("Menu_PlayNext"), c => CollectionActions.PlayNext(_s, c));
        Add(l.Get("Menu_PlayLast"), c => CollectionActions.PlayLast(_s, c));
        var addTo = new MenuFlyoutSubItem { Text = l.Get("Menu_AddToPlaylist") };
        Flyout.Items.Add(addTo);
        if (goToArtist)
        {
            _goToArtist = new MenuFlyoutItem { Text = l.Get("Menu_GoToArtist") };
            _goToArtist.Click += OnGoToArtist;
            Flyout.Items.Add(_goToArtist);
        }

        if (extraKey is not null && extra is not null)
        {
            Flyout.Items.Add(new MenuFlyoutSeparator());
            var item = new MenuFlyoutItem { Text = l.Get(extraKey) };
            item.Click += (_, _) =>
            {
                if (_item is not null)
                {
                    extra(_item);
                }
            };
            Flyout.Items.Add(item);
        }

        Flyout.Opening += async (_, _) =>
        {
            _item = Lists.ItemOf<object>(Flyout);
            var collection = _item is null ? null : _collectionOf(_item);
            var enabled = collection is not null;
            foreach (var entry in Flyout.Items.OfType<MenuFlyoutItem>())
            {
                entry.IsEnabled = enabled;
            }

            addTo.IsEnabled = enabled;
            if (_goToArtist is not null)
            {
                _goToArtist.IsEnabled = CardActions.HasArtist(_item as AlbumItem);
            }

            if (collection is not null)
            {
                try
                {
                    await PlaylistMenu.FillAsync(addTo, _s, () => PlaylistActions.TracksOfAsync(_s, collection));
                }
                catch (Exception e)
                {
                    // async void: an exception would end the app. The submenu keeps what it had.
                    AppLog.Write("filling Add to Playlist… failed", e);
                }
            }
        };
    }

    public MenuFlyout Flyout { get; }

    /// <summary>Go to Artist: the library's artist of the album's album artist.</summary>
    private async void OnGoToArtist(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        try
        {
            if (await CardActions.ArtistAsync(_s, _item as AlbumItem) is { } artist)
            {
                AppHost.Window.Navigate(typeof(ArtistPage), artist);
            }
        }
        catch (Exception ex)
        {
            // async void: an exception would end the app.
            AppLog.Write("Go to Artist failed", ex);
        }
    }

    private void Add(string text, Action<Collection> action)
    {
        var item = new MenuFlyoutItem { Text = text };
        item.Click += (_, _) =>
        {
            if (_item is not null && _collectionOf(_item) is { } collection)
            {
                action(collection);
            }
        };
        Flyout.Items.Add(item);
    }
}
