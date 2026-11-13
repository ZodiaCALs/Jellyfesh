using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Shell;

/// <summary>
/// The context menu of a track row: play next, add to the queue, add to a playlist (or a new one),
/// favourite, and an optional removal (from a playlist, from favourites). It only sends commands.
/// One menu serves every row of a list; it reads its row when it opens.
/// </summary>
internal sealed class TrackMenu
{
    private readonly ShellServices _s;
    private readonly MenuFlyoutItem _favorite;
    private readonly MenuFlyoutSubItem _playlists;
    private readonly MenuFlyoutItem? _remove;
    private readonly Action<TrackItem>? _onRemove;
    private TrackItem? _item;

    public TrackMenu(ShellServices services, string? removeKey = null, Action<TrackItem>? onRemove = null)
    {
        _s = services;
        _onRemove = onRemove;
        var l = services.L;
        Flyout = new MenuFlyout();
        Flyout.Items.Add(Item(l.Get("Menu_PlayNext"), () => TrackActions.PlayNext(_s, Items())));
        Flyout.Items.Add(Item(l.Get("Menu_AddToQueue"), () => TrackActions.PlayLast(_s, Items())));
        _playlists = new MenuFlyoutSubItem { Text = l.Get("Menu_AddToPlaylist") };
        Flyout.Items.Add(_playlists);
        _favorite = Item(l.Get("Menu_Favorite"), () =>
        {
            if (_item is not null)
            {
                TrackActions.ToggleFavorite(_s, _item);
            }
        });
        Flyout.Items.Add(_favorite);
        if (removeKey is not null && onRemove is not null)
        {
            Flyout.Items.Add(new MenuFlyoutSeparator());
            _remove = Item(l.Get(removeKey), () =>
            {
                if (_item is not null)
                {
                    _onRemove?.Invoke(_item);
                }
            });
            Flyout.Items.Add(_remove);
        }

        Flyout.Opening += OnOpening;
    }

    public MenuFlyout Flyout { get; }

    private IEnumerable<TrackItem> Items() => _item is null ? [] : [_item];

    private static MenuFlyoutItem Item(string text, Action action)
    {
        var item = new MenuFlyoutItem { Text = text };
        item.Click += (_, _) => action();
        return item;
    }

    private async void OnOpening(object? sender, object e)
    {
        _item = Lists.ItemOf<TrackItem>(Flyout);
        var l = _s.L;
        var playable = _item is { IsLoaded: true, IsPlayable: true };
        foreach (var entry in Flyout.Items.OfType<MenuFlyoutItem>())
        {
            entry.IsEnabled = _item is { IsLoaded: true };
        }

        Flyout.Items.OfType<MenuFlyoutItem>().Take(2).ToList().ForEach(i => i.IsEnabled = playable);
        _favorite.Text = l.Get(_item?.IsFavorite == true ? "Menu_Unfavorite" : "Menu_Favorite");
        _favorite.IsEnabled = _item?.Ref is TrackRef.Library or TrackRef.Source;
        _playlists.IsEnabled = _item is { IsLoaded: true, Ref: not null };

        _playlists.Items.Clear();
        var create = new MenuFlyoutItem { Text = l.Get("Menu_NewPlaylist") };
        create.Click += async (_, _) =>
        {
            var item = _item;
            var name = await AppHost.Platform.PromptTextAsync(l.Get("Playlist_NewTitle"), l.Get("Playlist_DefaultName"), l.Get("Action_Create"));
            if (!string.IsNullOrWhiteSpace(name) && item?.Ref is { } r)
            {
                _s.Send(new CoreCommand.CreatePlaylist(name.Trim(), [r]));
            }
        };
        _playlists.Items.Add(create);
        var lists = await _s.QueryAsync(c => c.Playlists());
        if (lists.IsOk && lists.Value!.Count > 0)
        {
            _playlists.Items.Add(new MenuFlyoutSeparator());
            foreach (var p in lists.Value!)
            {
                var id = p.Id;
                _playlists.Items.Add(Item(p.Name, () => TrackActions.AddToPlaylist(_s, id, Items())));
            }
        }
    }
}
