using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Shell;

/// <summary>
/// The context menu of an album, artist or playlist tile: Play, Shuffle, Play Next, Play Last, and
/// an optional extra action (Delete Playlist…). One menu serves a whole grid.
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

        Flyout.Opening += (_, _) =>
        {
            _item = Lists.ItemOf<object>(Flyout);
            var enabled = _item is not null && _collectionOf(_item) is not null;
            foreach (var entry in Flyout.Items.OfType<MenuFlyoutItem>())
            {
                entry.IsEnabled = enabled;
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
