using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Shell;

/// <summary>
/// The context menu of an album, artist or playlist tile or row: Play, Shuffle, Play Next, Play
/// Last, Add to Playlist… (docs/parity.md N3), and an optional extra action (Delete Playlist…). One
/// menu serves a whole grid or list.
/// </summary>
internal sealed class CollectionMenu
{
    private readonly ShellServices _s;
    private readonly Func<object, Collection?> _collectionOf;
    private object? _item;

    public CollectionMenu(ShellServices services, Func<object, Collection?> collectionOf, string? extraKey = null, Action<object>? extra = null)
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
