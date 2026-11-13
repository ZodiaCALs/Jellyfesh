using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Collections;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>Actions every track list offers (play from here, play next, add to a playlist, favourite).</summary>
public static class TrackActions
{
    /// <summary>How many tracks after the chosen one a flat list queues.</summary>
    public const uint FollowingTracks = 500;

    public static void PlayNext(ShellServices s, IEnumerable<TrackItem> items) =>
        Enqueue(s, items, playNext: true);

    public static void PlayLast(ShellServices s, IEnumerable<TrackItem> items) =>
        Enqueue(s, items, playNext: false);

    public static void ToggleFavorite(ShellServices s, TrackItem item)
    {
        if (item.Ref is { } r)
        {
            s.Send(new CoreCommand.SetFavorite(r, !item.IsFavorite));
            item.IsFavorite = !item.IsFavorite;
        }
    }

    public static void AddToPlaylist(ShellServices s, long playlistId, IEnumerable<TrackItem> items)
    {
        var refs = items.Select(i => i.Ref).OfType<TrackRef>().ToArray();
        if (refs.Length > 0)
        {
            s.Send(new CoreCommand.AddToPlaylist(playlistId, refs));
        }
    }

    private static void Enqueue(ShellServices s, IEnumerable<TrackItem> items, bool playNext)
    {
        var refs = items.Where(i => i.IsPlayable).Select(i => i.Ref).OfType<TrackRef>().ToArray();
        if (refs.Length > 0)
        {
            s.Send(new CoreCommand.Enqueue(refs, playNext));
        }
    }
}

/// <summary>Playing a whole album, artist, playlist or source container, now or after the current track.</summary>
public static class CollectionActions
{
    public static void Play(ShellServices s, Collection collection, bool shuffle = false) =>
        s.Send(new CoreCommand.PlayCollection(collection, 0, shuffle));

    public static void PlayNext(ShellServices s, Collection collection) =>
        s.Send(new CoreCommand.EnqueueCollection(collection, true));

    public static void PlayLast(ShellServices s, Collection collection) =>
        s.Send(new CoreCommand.EnqueueCollection(collection, false));
}

public sealed partial class HomeViewModel(ShellServices services) : PageViewModel(services)
{
    private const uint SectionSize = 12;

    [ObservableProperty]
    private bool _playStatsAvailable = true;

    public ObservableCollection<TrackItem> RecentlyAdded { get; } = [];

    public ObservableCollection<TrackItem> RecentlyPlayed { get; } = [];

    public ObservableCollection<TrackItem> MostPlayed { get; } = [];

    public ObservableCollection<PlaylistItem> Playlists { get; } = [];

    [RelayCommand]
    private void Play(TrackItem? item)
    {
        if (item?.Ref is { } r)
        {
            S.Send(new CoreCommand.PlayTracks([r], 0));
        }
    }

    protected override async Task LoadCoreAsync()
    {
        var l = S.L;
        var added = await S.QueryAsync(c => c.RecentlyAdded(SectionSize));
        Fill(RecentlyAdded, added.IsOk ? added.Value!.Select((t, i) => Item(i, x => x.Load(t))) : []);
        var played = await S.QueryAsync(c => c.RecentlyPlayed(SectionSize));
        var most = await S.QueryAsync(c => c.MostPlayed(SectionSize));
        PlayStatsAvailable = !played.IsNotAvailable && !most.IsNotAvailable;
        Fill(RecentlyPlayed, played.IsOk ? played.Value!.Select((r, i) => Item(i, x => x.Load(r))) : []);
        Fill(MostPlayed, most.IsOk ? most.Value!.Select((r, i) => Item(i, x => x.Load(r))) : []);
        var playlists = await S.QueryAsync(c => c.Playlists());
        Fill(Playlists, playlists.IsOk ? playlists.Value!.Take(8).Select(p => new PlaylistItem(p, l)) : []);
        IsEmpty = RecentlyAdded.Count == 0 && RecentlyPlayed.Count == 0 && Playlists.Count == 0;
    }

    private TrackItem Item(int index, Action<TrackItem> load)
    {
        var item = new TrackItem(index) { Localizer = S.L };
        load(item);
        return item;
    }

    private static void Fill<T>(ObservableCollection<T> target, IEnumerable<T> items)
    {
        target.Clear();
        foreach (var item in items)
        {
            target.Add(item);
        }
    }
}

/// <summary>The album grid: every album as a placeholder at once, pages filled as they scroll in.</summary>
public sealed partial class AlbumsViewModel(ShellServices services) : PageViewModel(services)
{
    [ObservableProperty]
    private PagedList<AlbumItem, Album>? _albums;

    [ObservableProperty]
    private AlbumSort _sort = AlbumSort.Title;

    public IReadOnlyList<AlbumSort> Sorts { get; } = [AlbumSort.Title, AlbumSort.Artist, AlbumSort.Year, AlbumSort.Added];

    partial void OnSortChanged(AlbumSort value) => _ = LoadAsync();

    protected override async Task LoadCoreAsync()
    {
        var stats = await S.QueryAsync(c => c.Stats());
        if (!Accept(stats))
        {
            return;
        }

        var sort = Sort;
        var list = new PagedList<AlbumItem, Album>(
            (long)stats.Value!.Albums,
            i => new AlbumItem(i) { Localizer = S.L },
            (offset, limit) => S.Core.AlbumsSorted(sort, sort is AlbumSort.Added or AlbumSort.Year, (uint)offset, (uint)limit),
            S.Ui);
        list.Start();
        Albums?.Invalidate();
        Albums = list;
        IsEmpty = list.Items.Count == 0;
    }

    [RelayCommand]
    private void Play(AlbumItem? item)
    {
        if (item is { IsLoaded: true })
        {
            S.Send(new CoreCommand.PlayCollection(new Collection.Album(item.Id), 0, false));
        }
    }
}

public sealed partial class AlbumDetailViewModel : PageViewModel
{
    [ObservableProperty]
    private string _summary = string.Empty;

    /// <summary>Genre, year and song count under the title ("Ambient · 2024 · 12 songs").</summary>
    [ObservableProperty]
    private string _details = string.Empty;

    /// <summary>"12 songs, 48 minutes" under the last track.</summary>
    [ObservableProperty]
    private string _footer = string.Empty;

    public AlbumDetailViewModel(ShellServices services, Album album)
        : base(services)
    {
        Album = album;
        Header = new AlbumItem(album, services.L);
    }

    public Album Album { get; }

    public AlbumItem Header { get; }

    /// <summary>The album artist, shown in accent text; empty when unknown.</summary>
    public string ArtistName => Album.AlbumArtist ?? string.Empty;

    /// <summary>Raised when the artist link resolved to a library artist (the view opens its page).</summary>
    public event Action<Artist>? ArtistRequested;

    public ObservableCollection<TrackItem> Tracks { get; } = [];

    /// <summary>Discs, when the album has more than one: the view shows a heading per disc.</summary>
    public bool IsMultiDisc { get; private set; }

    protected override async Task LoadCoreAsync()
    {
        var tracks = await S.QueryAsync(c => c.AlbumTracks(Album.Id));
        if (!Accept(tracks))
        {
            return;
        }

        Tracks.Clear();
        var index = 0;
        foreach (var t in tracks.Value!)
        {
            var item = new TrackItem(index++) { Localizer = S.L };
            item.Load(t);
            Tracks.Add(item);
        }

        IsMultiDisc = tracks.Value!.Select(t => t.DiscNo).Distinct().Count() > 1;
        var total = tracks.Value!.Aggregate(0UL, (sum, t) => sum + (t.DurationMs ?? 0));
        var songs = S.L.Plural("Count_Songs", Tracks.Count);
        Summary = S.L.Format("Album_Summary", songs, TimeFormat.Total(total, S.L));
        Footer = S.L.Format("Album_Summary", songs, TimeFormat.LongTotal(total, S.L));
        var genre = tracks.Value!.Select(t => t.Genre).FirstOrDefault(g => !string.IsNullOrWhiteSpace(g));
        var year = Album.Year?.ToString(S.L.Culture);
        Details = string.Join(" · ", new[] { genre, year, songs }.Where(p => !string.IsNullOrEmpty(p)));
        IsEmpty = Tracks.Count == 0;
        MarkCurrent(S.Hub.State.NowPlaying);
    }

    protected override void OnSlices(SliceChange changes, CoreSnapshot state)
    {
        if (changes.HasFlag(SliceChange.NowPlaying))
        {
            MarkCurrent(state.NowPlaying);
        }
    }

    /// <summary>The row of the song playing shows the level glyph.</summary>
    private void MarkCurrent(NowPlayingInfo? now)
    {
        foreach (var item in Tracks)
        {
            item.IsCurrent = now is not null && item.Ref is { } r && r == now.Track;
        }
    }

    /// <summary>Opens the album artist: the library's artist of that name, found through the core's search.</summary>
    [RelayCommand]
    private async Task OpenArtist()
    {
        if (string.IsNullOrEmpty(ArtistName))
        {
            return;
        }

        var results = await S.QueryAsync(c => c.SearchLibrary(ArtistName, 20));
        var artist = results.Value?.Artists.FirstOrDefault(a => string.Equals(a.Name, ArtistName, StringComparison.Ordinal))
            ?? results.Value?.Artists.FirstOrDefault(a => string.Equals(a.Name, ArtistName, StringComparison.OrdinalIgnoreCase));
        if (artist is not null)
        {
            ArtistRequested?.Invoke(artist);
        }
    }

    [RelayCommand]
    private void Play() => S.Send(new CoreCommand.PlayCollection(new Collection.Album(Album.Id), 0, false));

    [RelayCommand]
    private void Shuffle() => S.Send(new CoreCommand.PlayCollection(new Collection.Album(Album.Id), 0, true));

    [RelayCommand]
    private void PlayNext() => S.Send(new CoreCommand.EnqueueCollection(new Collection.Album(Album.Id), true));

    [RelayCommand]
    private void PlayLast() => S.Send(new CoreCommand.EnqueueCollection(new Collection.Album(Album.Id), false));

    [RelayCommand]
    private void PlayTrack(TrackItem? item)
    {
        if (item is not null)
        {
            S.Send(new CoreCommand.PlayCollection(new Collection.Album(Album.Id), (uint)Math.Max(0, Tracks.IndexOf(item)), false));
        }
    }
}

public sealed partial class ArtistsViewModel(ShellServices services) : PageViewModel(services)
{
    [ObservableProperty]
    private PagedList<ArtistItem, Artist>? _artists;

    protected override async Task LoadCoreAsync()
    {
        var stats = await S.QueryAsync(c => c.Stats());
        if (!Accept(stats))
        {
            return;
        }

        var list = new PagedList<ArtistItem, Artist>(
            (long)stats.Value!.Artists,
            i => new ArtistItem(i) { Localizer = S.L },
            (offset, limit) => S.Core.Artists((uint)offset, (uint)limit),
            S.Ui);
        list.Start();
        Artists?.Invalidate();
        Artists = list;
        IsEmpty = list.Items.Count == 0;
    }
}

public sealed partial class ArtistDetailViewModel(ShellServices services, Artist artist) : PageViewModel(services)
{
    private const uint TopSongs = 20;

    public Artist Artist { get; } = artist;

    public string Name => Artist.Name;

    public ObservableCollection<AlbumItem> Albums { get; } = [];

    public ObservableCollection<TrackItem> Songs { get; } = [];

    protected override async Task LoadCoreAsync()
    {
        var albums = await S.QueryAsync(c => c.ArtistAlbums(Artist.Id));
        Accept(albums);
        Albums.Clear();
        foreach (var a in albums.Value ?? [])
        {
            Albums.Add(new AlbumItem(a, S.L));
        }

        var songs = await S.QueryAsync(c => c.Tracks(new TrackQuery(0, TopSongs, TrackSort.Album, false, null, Artist.Id)));
        Songs.Clear();
        var index = 0;
        foreach (var t in songs.Value ?? [])
        {
            var item = new TrackItem(index++) { Localizer = S.L };
            item.Load(t);
            Songs.Add(item);
        }

        IsEmpty = Albums.Count == 0 && Songs.Count == 0;
    }

    [RelayCommand]
    private void Play() => S.Send(new CoreCommand.PlayCollection(new Collection.Artist(Artist.Id), 0, false));

    [RelayCommand]
    private void Shuffle() => S.Send(new CoreCommand.PlayCollection(new Collection.Artist(Artist.Id), 0, true));

    [RelayCommand]
    private void PlayTrack(TrackItem? item)
    {
        if (item?.Ref is not null)
        {
            S.Send(new CoreCommand.PlayTracks(Songs.Select(s => s.Ref).OfType<TrackRef>().ToArray(), (uint)Math.Max(0, Songs.IndexOf(item))));
        }
    }
}

/// <summary>Every song, virtualised and sorted by the core.</summary>
public sealed partial class SongsViewModel(ShellServices services) : PageViewModel(services)
{
    [ObservableProperty]
    private PagedList<TrackItem, Track>? _songs;

    [ObservableProperty]
    private TrackSort _sort = TrackSort.Artist;

    public IReadOnlyList<TrackSort> Sorts { get; } = [TrackSort.Artist, TrackSort.Title, TrackSort.Album, TrackSort.Year, TrackSort.Duration, TrackSort.Added];

    partial void OnSortChanged(TrackSort value) => _ = LoadAsync();

    protected override async Task LoadCoreAsync()
    {
        var stats = await S.QueryAsync(c => c.Stats());
        if (!Accept(stats))
        {
            return;
        }

        var sort = Sort;
        var descending = sort == TrackSort.Added;
        var list = new PagedList<TrackItem, Track>(
            (long)stats.Value!.Tracks,
            i => new TrackItem(i) { Localizer = S.L },
            (offset, limit) => S.Core.Tracks(new TrackQuery((uint)offset, (uint)limit, sort, descending)),
            S.Ui);
        list.Start();
        Songs?.Invalidate();
        Songs = list;
        IsEmpty = list.Items.Count == 0;
    }

    /// <summary>Plays the song and the ones after it in this order.</summary>
    [RelayCommand]
    private async Task PlayFrom(TrackItem? item)
    {
        if (item is null || item.Index < 0)
        {
            return;
        }

        var sort = Sort;
        var page = await S.QueryAsync(c => c.Tracks(new TrackQuery((uint)item.Index, TrackActions.FollowingTracks, sort, sort == TrackSort.Added)));
        if (page.IsOk && page.Value!.Count > 0)
        {
            S.Send(new CoreCommand.PlayTracks(page.Value!.Select(t => (TrackRef)new TrackRef.Library(t.Id)).ToArray(), 0));
        }
    }

    /// <summary>Shuffles the most recently added songs (a command carries its track list, so it is capped).</summary>
    [RelayCommand]
    private async Task Shuffle()
    {
        var page = await S.QueryAsync(c => c.Tracks(new TrackQuery(0, TrackActions.FollowingTracks * 4, TrackSort.Added, true)));
        if (page.IsOk && page.Value!.Count > 0)
        {
            S.Send(new CoreCommand.PlayCollection(new Collection.Tracks(page.Value!.Select(t => (TrackRef)new TrackRef.Library(t.Id)).ToArray()), 0, true));
        }
    }
}

public sealed partial class PlaylistsViewModel(ShellServices services) : PageViewModel(services)
{
    public ObservableCollection<PlaylistItem> Playlists { get; } = [];

    protected override async Task LoadCoreAsync()
    {
        var result = await S.QueryAsync(c => c.Playlists());
        Accept(result);
        Playlists.Clear();
        foreach (var p in result.Value ?? [])
        {
            Playlists.Add(new PlaylistItem(p, S.L));
        }

        IsEmpty = Playlists.Count == 0 && !IsNotAvailable;
    }

    [RelayCommand]
    private async Task Create()
    {
        var name = await S.Platform.PromptTextAsync(S.L.Get("Playlist_NewTitle"), S.L.Get("Playlist_DefaultName"), S.L.Get("Action_Create"));
        if (!string.IsNullOrWhiteSpace(name))
        {
            S.Send(new CoreCommand.CreatePlaylist(name.Trim(), []));
        }
    }

    [RelayCommand]
    private async Task Import()
    {
        var path = await S.Platform.PickFileAsync([".m3u", ".m3u8"]);
        if (path is not null)
        {
            S.Send(new CoreCommand.ImportPlaylist(path));
        }
    }

    [RelayCommand]
    private void Play(PlaylistItem? item)
    {
        if (item is not null)
        {
            S.Send(new CoreCommand.PlayCollection(new Collection.Playlist(item.Id), 0, false));
        }
    }

    [RelayCommand]
    private async Task Delete(PlaylistItem? item)
    {
        if (item is not null && await S.Platform.ConfirmAsync(
                S.L.Format("Playlist_DeleteTitle", item.Name), S.L.Get("Playlist_DeleteBody"), S.L.Get("Action_Delete"), S.L.Get("Action_Cancel")))
        {
            S.Send(new CoreCommand.DeletePlaylist(item.Id));
        }
    }
}

public sealed partial class PlaylistDetailViewModel : PageViewModel
{
    [ObservableProperty]
    private string _name;

    [ObservableProperty]
    private string _summary = string.Empty;

    [ObservableProperty]
    private PagedList<TrackItem, PlaylistEntry>? _entries;

    public PlaylistDetailViewModel(ShellServices services, PlaylistSummary playlist)
        : base(services)
    {
        Playlist = playlist;
        _name = playlist.Name;
    }

    public PlaylistSummary Playlist { get; private set; }

    protected override async Task LoadCoreAsync()
    {
        var fresh = await S.QueryAsync(c => c.Playlist(Playlist.Id));
        if (!Accept(fresh))
        {
            return;
        }

        if (fresh.Value is null)
        {
            IsEmpty = true; // deleted
            return;
        }

        Playlist = fresh.Value;
        Name = Playlist.Name;
        Summary = S.L.Format("Playlist_Summary", S.L.Plural("Count_Songs", Playlist.EntryCount), TimeFormat.Total(Playlist.DurationMs, S.L));
        var id = Playlist.Id;
        var list = new PagedList<TrackItem, PlaylistEntry>(
            Playlist.EntryCount,
            i => new TrackItem(i) { Localizer = S.L },
            (offset, limit) => S.Core.PlaylistEntries(id, (uint)offset, (uint)limit),
            S.Ui);
        list.Start();
        Entries?.Invalidate();
        Entries = list;
        IsEmpty = Playlist.EntryCount == 0;
    }

    [RelayCommand]
    private void Play() => S.Send(new CoreCommand.PlayCollection(new Collection.Playlist(Playlist.Id), 0, false));

    [RelayCommand]
    private void Shuffle() => S.Send(new CoreCommand.PlayCollection(new Collection.Playlist(Playlist.Id), 0, true));

    [RelayCommand]
    private void PlayTrack(TrackItem? item)
    {
        if (item is { Index: >= 0 })
        {
            S.Send(new CoreCommand.PlayCollection(new Collection.Playlist(Playlist.Id), (uint)item.Index, false));
        }
    }

    [RelayCommand]
    private void RemoveEntry(TrackItem? item)
    {
        if (item?.EntryId is { } entry)
        {
            S.Send(new CoreCommand.RemoveFromPlaylist(Playlist.Id, [entry]));
        }
    }

    /// <summary>The list dropped an entry at <paramref name="newIndex"/>.</summary>
    public void MoveEntry(TrackItem item, int newIndex)
    {
        if (item.EntryId is { } entry && newIndex >= 0)
        {
            S.Send(new CoreCommand.MovePlaylistEntry(Playlist.Id, entry, (uint)newIndex));
        }
    }

    [RelayCommand]
    private async Task Rename()
    {
        var name = await S.Platform.PromptTextAsync(S.L.Get("Playlist_RenameTitle"), Name, S.L.Get("Action_Rename"));
        if (!string.IsNullOrWhiteSpace(name) && name.Trim() != Name)
        {
            S.Send(new CoreCommand.RenamePlaylist(Playlist.Id, name.Trim()));
        }
    }

    [RelayCommand]
    private async Task Export()
    {
        var path = await S.Platform.PickSaveFileAsync(Name, ".m3u8");
        if (path is null)
        {
            return;
        }

        var id = Playlist.Id;
        var done = await S.QueryAsync(c =>
        {
            c.ExportPlaylist(id, path);
            return true;
        });
        if (done.IsOk)
        {
            S.Notifications.Success(S.L.Get("Playlist_Exported"), path);
        }
    }
}

public sealed partial class FavoritesViewModel(ShellServices services) : PageViewModel(services)
{
    [ObservableProperty]
    private PagedList<TrackItem, TrackRow>? _favorites;

    protected override Task LoadCoreAsync()
    {
        var list = new PagedList<TrackItem, TrackRow>(
            null,
            i => new TrackItem(i) { Localizer = S.L },
            (offset, limit) => S.Core.Favorites((uint)offset, (uint)limit),
            S.Ui);
        list.Failed += e => IsNotAvailable = e.IsNotYetAvailable;
        list.PageLoaded += () => IsEmpty = list.Items.Count == 0;
        list.Start();
        Favorites?.Invalidate();
        Favorites = list;
        return Task.CompletedTask;
    }

    [RelayCommand]
    private void Play() => S.Send(new CoreCommand.PlayCollection(new Collection.Favorites(), 0, false));

    [RelayCommand]
    private void Shuffle() => S.Send(new CoreCommand.PlayCollection(new Collection.Favorites(), 0, true));

    [RelayCommand]
    private void PlayTrack(TrackItem? item)
    {
        if (item is { Index: >= 0 })
        {
            S.Send(new CoreCommand.PlayCollection(new Collection.Favorites(), (uint)item.Index, false));
        }
    }

    [RelayCommand]
    private void Unfavorite(TrackItem? item)
    {
        if (item?.Ref is { } r)
        {
            S.Send(new CoreCommand.SetFavorite(r, false));
        }
    }
}
