using Jellyfesh.App.Views;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Shell;

/// <summary>
/// The song menu (docs/ui.md): Play Next, Play Last, Add to Playlist…, Add to Favourites or Remove
/// from Favourites, Go to Album, Go to Artist, Open in {Service} (songs with a link) and Show in
/// Explorer, and an optional removal (from a playlist, from favourites). A remote source's songs
/// are never queued, so they have no Play Next, Play Last or Add to Playlist… (ADR-0018). It only
/// sends commands and opens pages. One menu serves every row of a list; it reads its row when it
/// opens.
/// </summary>
internal sealed class TrackMenu
{
    private readonly ShellServices _s;
    private readonly MenuFlyoutItem _playNext;
    private readonly MenuFlyoutItem _playLast;
    private readonly MenuFlyoutSubItem _playlists;
    private readonly MenuFlyoutItem _favorite;
    private readonly MenuFlyoutItem _goToAlbum;
    private readonly MenuFlyoutItem _goToArtist;
    private readonly MenuFlyoutItem _makeLyrics;
    private readonly MenuFlyoutItem _openLink;
    private readonly MenuFlyoutItem _showFile;
    private readonly MenuFlyoutItem? _remove;
    private TrackItem? _item;

    public TrackMenu(ShellServices services, string? removeKey = null, Action<TrackItem>? onRemove = null)
    {
        _s = services;
        var l = services.L;
        Flyout = new MenuFlyout();
        _playNext = Item(l.Get("Menu_PlayNext"), () => TrackActions.PlayNext(_s, Items()));
        _playLast = Item(l.Get("Menu_PlayLast"), () => TrackActions.PlayLast(_s, Items()));
        _playlists = new MenuFlyoutSubItem { Text = l.Get("Menu_AddToPlaylist") };
        _favorite = Item(l.Get("Menu_Favorite"), () =>
        {
            if (_item is not null)
            {
                TrackActions.ToggleFavorite(_s, _item);
            }
        });
        _goToAlbum = Item(l.Get("Menu_GoToAlbum"), GoToAlbum);
        _goToArtist = Item(l.Get("Menu_GoToArtist"), GoToArtist);
        _makeLyrics = Item(l.Get("AiLyrics_Create"), MakeLyrics);
        _openLink = Item(string.Empty, () => ItemLinks.Open(_s, _item?.Link));
        _showFile = Item(l.Get("Menu_ShowInExplorer"), ShowFile);
        Flyout.Items.Add(_playNext);
        Flyout.Items.Add(_playLast);
        Flyout.Items.Add(_playlists);
        Flyout.Items.Add(_favorite);
        Flyout.Items.Add(new MenuFlyoutSeparator());
        Flyout.Items.Add(_goToAlbum);
        Flyout.Items.Add(_goToArtist);
        Flyout.Items.Add(_makeLyrics);
        Flyout.Items.Add(_openLink);
        Flyout.Items.Add(_showFile);
        if (removeKey is not null && onRemove is not null)
        {
            Flyout.Items.Add(new MenuFlyoutSeparator());
            _remove = Item(l.Get(removeKey), () =>
            {
                if (_item is not null)
                {
                    onRemove(_item);
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

    private async void GoToAlbum()
    {
        if (_item is { } item && await TrackPlaces.AlbumAsync(_s, item) is { } album)
        {
            AppHost.Window.Navigate(typeof(AlbumPage), album);
        }
    }

    private async void GoToArtist()
    {
        if (_item is { } item && await TrackPlaces.ArtistAsync(_s, item) is { } artist)
        {
            AppHost.Window.Navigate(typeof(ArtistPage), artist);
        }
    }

    /// <summary>Create Synced Lyrics for a library song (after what AI Lyrics needs first).</summary>
    private async void MakeLyrics()
    {
        if (_item?.Ref is TrackRef.Library song)
        {
            try
            {
                await AppHost.App.AiLyrics.CreateFor(new SongRef.Library(song.Id));
            }
            catch (Exception e)
            {
                // async void: an exception would end the app.
                AppLog.Write("Create Synced Lyrics failed", e);
            }
        }
    }

    private async void ShowFile()
    {
        if (_item is { } item)
        {
            await TrackPlaces.ShowInExplorerAsync(_s, item);
        }
    }

    private async void OnOpening(object? sender, object e)
    {
        _item = Lists.ItemOf<TrackItem>(Flyout);
        var loaded = _item is { IsLoaded: true };
        var playable = _item is { IsLoaded: true, IsPlayable: true };
        var queues = _item is null || TrackActions.CanQueue(_s, _item);
        var queueVisibility = queues ? Visibility.Visible : Visibility.Collapsed;
        _playNext.Visibility = queueVisibility;
        _playLast.Visibility = queueVisibility;
        _playlists.Visibility = queueVisibility;
        _playNext.IsEnabled = playable;
        _playLast.IsEnabled = playable;
        _openLink.Visibility = _item?.Link is null ? Visibility.Collapsed : Visibility.Visible;
        _openLink.Text = ItemLinks.OpenInLabel(_s.L, ItemLinks.ServiceName(_s.Hub.State.Sources, _item?.SourceId));
        _favorite.Text = _s.L.Get(_item?.IsFavorite == true ? "Menu_Unfavorite" : "Menu_Favorite");
        _favorite.IsEnabled = loaded && _item?.Ref is TrackRef.Library or TrackRef.Source;
        _goToAlbum.IsEnabled = loaded && TrackPlaces.HasPages(_item!);
        _goToArtist.IsEnabled = _goToAlbum.IsEnabled;

        // Library songs, while AI Lyrics is offered (docs/ui.md, "Song menu").
        _makeLyrics.Visibility = _item?.Ref is TrackRef.Library && AppHost.App.AiLyrics.OffersForSongs ? Visibility.Visible : Visibility.Collapsed;
        _showFile.IsEnabled = loaded && TrackPlaces.HasFile(_item!);
        if (_remove is not null)
        {
            _remove.IsEnabled = loaded;
        }

        var item = _item;
        _playlists.IsEnabled = item is { IsLoaded: true, Ref: not null };
        await PlaylistMenu.FillAsync(_playlists, _s, () => Task.FromResult<IReadOnlyList<TrackRef>>(item?.Ref is { } r ? [r] : []));
    }
}
