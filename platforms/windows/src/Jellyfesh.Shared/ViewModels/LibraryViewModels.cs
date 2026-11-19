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

    /// <summary>
    /// Whether Play Next, Play Last and Add to Playlist are offered for the song: not for a remote
    /// source's songs, which play on the person's own devices and never join the queue (ADR-0018).
    /// </summary>
    public static bool CanQueue(ShellServices s, TrackItem? item) =>
        item?.Ref is { } r && !ItemLinks.IsRemote(s.Hub.State.Sources, r);

    public static void AddToPlaylist(ShellServices s, long playlistId, IEnumerable<TrackItem> items)
    {
        var refs = items.Where(i => CanQueue(s, i)).Select(i => i.Ref).OfType<TrackRef>().ToArray();
        if (refs.Length > 0)
        {
            s.Send(new CoreCommand.AddToPlaylist(playlistId, refs));
        }
    }

    private static void Enqueue(ShellServices s, IEnumerable<TrackItem> items, bool playNext)
    {
        var refs = items.Where(i => i.IsPlayable && CanQueue(s, i)).Select(i => i.Ref).OfType<TrackRef>().ToArray();
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

/// <summary>Add Music Folder… (Home, the empty library pages) and Add Folder… (Settings › Library).</summary>
public static class LibraryActions
{
    /// <summary>Asks for a folder and adds it to the library; false when the person cancelled.</summary>
    public static async Task<bool> AddFolderAsync(ShellServices s)
    {
        var folder = await s.Platform.PickFolderAsync();
        if (folder is null)
        {
            return false;
        }

        s.Send(new CoreCommand.AddRoot(folder));
        return true;
    }
}

/// <summary>New Playlist… (it asks for the name first) and Add to Playlist… for songs, albums, artists and playlists.</summary>
public static class PlaylistActions
{
    /// <summary>The most songs one Add to Playlist… takes from an artist or a playlist.</summary>
    public const uint MaxTracks = 100_000;

    /// <summary>
    /// The songs of an album, an artist or a playlist in the core's order, for Add to Playlist… on a
    /// tile or a row (docs/parity.md N3); none for anything else.
    /// </summary>
    public static async Task<IReadOnlyList<TrackRef>> TracksOfAsync(ShellServices s, Collection collection)
    {
        switch (collection)
        {
            case Collection.Album album:
                var albumTracks = await s.QueryAsync(c => c.AlbumTracks(album.Id));
                return albumTracks.Value?.Select(t => (TrackRef)new TrackRef.Library(t.Id)).ToArray() ?? [];
            case Collection.Artist artist:
                var artistTracks = await s.QueryAsync(c => c.Tracks(new TrackQuery(0, MaxTracks, TrackSort.Album, false, null, artist.Id)));
                return artistTracks.Value?.Select(t => (TrackRef)new TrackRef.Library(t.Id)).ToArray() ?? [];
            case Collection.Playlist playlist:
                var entries = await s.QueryAsync(c => c.PlaylistEntries(playlist.Id, 0, MaxTracks));
                return entries.Value?.Select(e => e.Row.Track).ToArray() ?? [];
            default:
                return [];
        }
    }

    /// <summary>Asks for a name, then makes the playlist with <paramref name="tracks"/> (none for an empty one); nothing when cancelled.</summary>
    public static async Task CreateAsync(ShellServices s, IReadOnlyList<TrackRef> tracks)
    {
        var name = await s.Platform.PromptTextAsync(s.L.Get("Playlist_NewTitle"), s.L.Get("Playlist_DefaultName"), s.L.Get("Action_Create"));
        if (!string.IsNullOrWhiteSpace(name))
        {
            s.Send(new CoreCommand.CreatePlaylist(name.Trim(), tracks));
        }
    }

    public static void AddTo(ShellServices s, long playlistId, IReadOnlyList<TrackRef> tracks)
    {
        if (tracks.Count > 0)
        {
            s.Send(new CoreCommand.AddToPlaylist(playlistId, tracks));
        }
    }
}

/// <summary>Where a song lives, for the song menu's Go to Album, Go to Artist and Show in Explorer.</summary>
public static class TrackPlaces
{
    private const uint SearchLimit = 50;

    /// <summary>Library songs have an album and an artist page.</summary>
    public static bool HasPages(TrackItem item) => item.Ref is TrackRef.Library;

    /// <summary>Library songs and plain files have a file to show.</summary>
    public static bool HasFile(TrackItem item) => item.Ref is TrackRef.Library or TrackRef.File;

    /// <summary>The library album of a library song, or null.</summary>
    public static async Task<Album?> AlbumAsync(ShellServices s, TrackItem item)
    {
        if (item.Ref is not TrackRef.Library song)
        {
            return null;
        }

        var found = await s.QueryAsync(c =>
        {
            var track = c.Track(song.Id);
            return track is { AlbumId: { } albumId, Album: { Length: > 0 } title }
                ? c.SearchLibrary(title, SearchLimit).Albums.FirstOrDefault(a => a.Id == albumId)
                : null;
        });
        return found.Value;
    }

    /// <summary>The library artist of a library song (its artist, else its album artist), or null.</summary>
    public static async Task<Artist?> ArtistAsync(ShellServices s, TrackItem item)
    {
        if (item.Ref is not TrackRef.Library song)
        {
            return null;
        }

        var found = await s.QueryAsync(c => c.Track(song.Id) is { } track ? FindArtist(c, track.Artist ?? track.AlbumArtist) : null);
        return found.Value;
    }

    /// <summary>The song's file, or null for a song from a source.</summary>
    public static async Task<string?> FileAsync(ShellServices s, TrackItem item) => item.Ref switch
    {
        TrackRef.File file => file.Path,
        TrackRef.Library song => (await s.QueryAsync(c => c.Track(song.Id)?.Path)).Value,
        _ => null,
    };

    /// <summary>Show in Explorer: the song's file, selected in its folder.</summary>
    public static async Task ShowInExplorerAsync(ShellServices s, TrackItem item)
    {
        if (await FileAsync(s, item) is { } path)
        {
            s.Platform.ShowInExplorer(path);
        }
    }

    /// <summary>The library artist named <paramref name="name"/> (exactly, else ignoring case), through the core's search.</summary>
    internal static Artist? FindArtist(ICoreClient core, string? name)
    {
        if (string.IsNullOrEmpty(name))
        {
            return null;
        }

        var artists = core.SearchLibrary(name, SearchLimit).Artists;
        return artists.FirstOrDefault(a => string.Equals(a.Name, name, StringComparison.Ordinal))
            ?? artists.FirstOrDefault(a => string.Equals(a.Name, name, StringComparison.OrdinalIgnoreCase));
    }
}

/// <summary>
/// Home: the library's size, then Recently Added (albums), Recently Played and Most Played (songs);
/// with nothing in the library and no source signed in, the welcome instead.
/// </summary>
public sealed partial class HomeViewModel(ShellServices services) : PageViewModel(services)
{
    private const uint SectionSize = 12;
    private ulong? _songs;

    [ObservableProperty]
    private bool _playStatsAvailable = true;

    /// <summary>"1,204 songs · 96 albums · 41 artists"; empty while the library is empty.</summary>
    [ObservableProperty]
    private string _librarySize = string.Empty;

    /// <summary>The welcome instead of the shelves: nothing in the library and no source signed in.</summary>
    [ObservableProperty]
    private bool _showWelcome;

    public ObservableCollection<AlbumItem> RecentlyAdded { get; } = [];

    public ObservableCollection<TrackItem> RecentlyPlayed { get; } = [];

    public ObservableCollection<TrackItem> MostPlayed { get; } = [];

    [RelayCommand]
    private void Play(TrackItem? item)
    {
        if (item?.Ref is { } r)
        {
            S.Send(new CoreCommand.PlayTracks([r], 0));
        }
    }

    /// <summary>The welcome's Add Music Folder….</summary>
    [RelayCommand]
    private Task AddFolder() => LibraryActions.AddFolderAsync(S);

    protected override async Task LoadCoreAsync()
    {
        var l = S.L;
        var stats = await S.QueryAsync(c => c.Stats());
        var st = stats.Value;
        _songs = st?.Tracks ?? 0;
        LibrarySize = st is { Tracks: > 0 }
            ? l.Format("Home_LibrarySize", l.Plural("Count_Songs", (long)st.Tracks), l.Plural("Count_Albums", (long)st.Albums), l.Plural("Count_Artists", (long)st.Artists))
            : string.Empty;
        var added = await S.QueryAsync(c => c.AlbumsSorted(AlbumSort.Added, true, 0, SectionSize));
        Fill(RecentlyAdded, added.IsOk ? added.Value!.Select(a => new AlbumItem(a, l)) : []);
        var played = await S.QueryAsync(c => c.RecentlyPlayed(SectionSize));
        var most = await S.QueryAsync(c => c.MostPlayed(SectionSize));
        PlayStatsAvailable = !played.IsNotAvailable && !most.IsNotAvailable;
        Fill(RecentlyPlayed, played.IsOk ? played.Value!.Select((r, i) => Item(i, x => x.Load(r))) : []);
        Fill(MostPlayed, most.IsOk ? most.Value!.Select((r, i) => Item(i, x => x.Load(r))) : []);
        IsEmpty = RecentlyAdded.Count == 0 && RecentlyPlayed.Count == 0 && MostPlayed.Count == 0;
        UpdateWelcome(S.Hub.State.Sources);
    }

    protected override void OnSlices(SliceChange changes, CoreSnapshot state)
    {
        if (changes.HasFlag(SliceChange.Sources))
        {
            UpdateWelcome(state.Sources); // signing in to a source ends the welcome
        }
    }

    private void UpdateWelcome(SourcesState sources) =>
        ShowWelcome = _songs == 0 && !sources.Plugins.Any(p => p.Account is not null);

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
    private bool _choosing;

    [ObservableProperty]
    private PagedList<AlbumItem, Album>? _albums;

    [ObservableProperty]
    private AlbumSort _sort = AlbumSort.Title;

    /// <summary>The sort menu's Descending; each order starts in its own direction (newest first for Year and Recently Added).</summary>
    [ObservableProperty]
    private bool _descending;

    /// <summary>The sort menu, in the order every app lists it: Title, Artist, Year, Recently Added.</summary>
    public IReadOnlyList<AlbumSort> Sorts { get; } = [AlbumSort.Title, AlbumSort.Artist, AlbumSort.Year, AlbumSort.Added];

    /// <summary>Whether an order starts descending: newest first for Year and Recently Added, A to Z otherwise.</summary>
    public static bool DescendsFirst(AlbumSort sort) => sort is AlbumSort.Year or AlbumSort.Added;

    partial void OnSortChanged(AlbumSort value)
    {
        _choosing = true;
        Descending = DescendsFirst(value);
        _choosing = false;
        _ = LoadAsync();
    }

    partial void OnDescendingChanged(bool value)
    {
        if (!_choosing)
        {
            _ = LoadAsync();
        }
    }

    /// <summary>The empty page's Add Music Folder….</summary>
    [RelayCommand]
    private Task AddFolder() => LibraryActions.AddFolderAsync(S);

    protected override async Task LoadCoreAsync()
    {
        var stats = await S.QueryAsync(c => c.Stats());
        if (!Accept(stats))
        {
            return;
        }

        var sort = Sort;
        var descending = Descending;
        var list = new PagedList<AlbumItem, Album>(
            (long)stats.Value!.Albums,
            i => new AlbumItem(i) { Localizer = S.L },
            (offset, limit) => S.Core.AlbumsSorted(sort, descending, (uint)offset, (uint)limit),
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

        var name = ArtistName;
        var artist = await S.QueryAsync(c => TrackPlaces.FindArtist(c, name));
        if (artist.Value is { } found)
        {
            ArtistRequested?.Invoke(found);
        }
    }

    /// <summary>The album's songs, for More › Add to Playlist….</summary>
    public async Task<IReadOnlyList<TrackRef>> TracksAsync()
    {
        if (Tracks.Count > 0)
        {
            return Tracks.Select(t => t.Ref).OfType<TrackRef>().ToArray();
        }

        var id = Album.Id;
        var tracks = await S.QueryAsync(c => c.AlbumTracks(id));
        return tracks.Value?.Select(t => (TrackRef)new TrackRef.Library(t.Id)).ToArray() ?? [];
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

    /// <summary>The empty page's Add Music Folder….</summary>
    [RelayCommand]
    private Task AddFolder() => LibraryActions.AddFolderAsync(S);

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
    private void PlayNext() => CollectionActions.PlayNext(S, new Collection.Artist(Artist.Id));

    [RelayCommand]
    private void PlayLast() => CollectionActions.PlayLast(S, new Collection.Artist(Artist.Id));

    /// <summary>All of the artist's songs, for More › Add to Playlist….</summary>
    public async Task<IReadOnlyList<TrackRef>> TracksAsync()
    {
        var id = Artist.Id;
        var limit = Math.Max(Artist.TrackCount, TopSongs);
        var tracks = await S.QueryAsync(c => c.Tracks(new TrackQuery(0, limit, TrackSort.Album, false, null, id)));
        return tracks.Value?.Select(t => (TrackRef)new TrackRef.Library(t.Id)).ToArray() ?? [];
    }

    [RelayCommand]
    private void PlayTrack(TrackItem? item)
    {
        if (item?.Ref is not null)
        {
            S.Send(new CoreCommand.PlayTracks(Songs.Select(s => s.Ref).OfType<TrackRef>().ToArray(), (uint)Math.Max(0, Songs.IndexOf(item))));
        }
    }
}

/// <summary>Every song, virtualised and sorted by the core: Play and Shuffle, then the list.</summary>
public sealed partial class SongsViewModel(ShellServices services) : PageViewModel(services)
{
    /// <summary>How many songs Play and Shuffle queue (a command carries its track list, so it is capped).</summary>
    public const uint WholeList = TrackActions.FollowingTracks * 4;

    private bool _choosing;

    [ObservableProperty]
    private PagedList<TrackItem, Track>? _songs;

    [ObservableProperty]
    // Artist first, as in the macOS and Android apps and the web (docs/parity.md V6).
    private TrackSort _sort = TrackSort.Artist;

    /// <summary>The sort menu's Descending; each order starts in its own direction (newest first for Year and Recently Added).</summary>
    [ObservableProperty]
    private bool _descending;

    /// <summary>The sort menu, in the order every app lists it: Title, Artist, Album, Year, Recently Added.</summary>
    public IReadOnlyList<TrackSort> Sorts { get; } = [TrackSort.Title, TrackSort.Artist, TrackSort.Album, TrackSort.Year, TrackSort.Added];

    /// <summary>Whether an order starts descending: newest first for Year and Recently Added, A to Z otherwise.</summary>
    public static bool DescendsFirst(TrackSort sort) => sort is TrackSort.Year or TrackSort.Added;

    partial void OnSortChanged(TrackSort value)
    {
        _choosing = true;
        Descending = DescendsFirst(value);
        _choosing = false;
        _ = LoadAsync();
    }

    partial void OnDescendingChanged(bool value)
    {
        if (!_choosing)
        {
            _ = LoadAsync();
        }
    }

    protected override async Task LoadCoreAsync()
    {
        var stats = await S.QueryAsync(c => c.Stats());
        if (!Accept(stats))
        {
            return;
        }

        var sort = Sort;
        var descending = Descending;
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

        var refs = await PageAsync((uint)item.Index, TrackActions.FollowingTracks);
        if (refs.Length > 0)
        {
            S.Send(new CoreCommand.PlayTracks(refs, 0));
        }
    }

    /// <summary>Play: the songs from the top, in this order.</summary>
    [RelayCommand]
    private async Task Play()
    {
        var refs = await PageAsync(0, WholeList);
        if (refs.Length > 0)
        {
            S.Send(new CoreCommand.PlayTracks(refs, 0));
        }
    }

    /// <summary>Shuffle: the same songs in random order.</summary>
    [RelayCommand]
    private async Task Shuffle()
    {
        var refs = await PageAsync(0, WholeList);
        if (refs.Length > 0)
        {
            S.Send(new CoreCommand.PlayCollection(new Collection.Tracks(refs), 0, true));
        }
    }

    /// <summary>The empty page's Add Music Folder….</summary>
    [RelayCommand]
    private Task AddFolder() => LibraryActions.AddFolderAsync(S);

    private async Task<TrackRef[]> PageAsync(uint offset, uint limit)
    {
        var query = new TrackQuery(offset, limit, Sort, Descending);
        var page = await S.QueryAsync(c => c.Tracks(query));
        return page.IsOk ? page.Value!.Select(t => (TrackRef)new TrackRef.Library(t.Id)).ToArray() : [];
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

    /// <summary>New Playlist… asks for the name before it makes anything.</summary>
    [RelayCommand]
    private Task Create() => PlaylistActions.CreateAsync(S, []);

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
                S.L.Format("Playlist_DeleteTitle", item.Name), S.L.Get("Playlist_DeleteBody"), S.L.Get("Playlist_DeleteConfirm"), S.L.Get("Action_Cancel")))
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

    /// <summary>Raised after Delete Playlist… removed this playlist (the page goes back).</summary>
    public event Action? Deleted;

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
    private void PlayNext() => CollectionActions.PlayNext(S, new Collection.Playlist(Playlist.Id));

    [RelayCommand]
    private void PlayLast() => CollectionActions.PlayLast(S, new Collection.Playlist(Playlist.Id));

    [RelayCommand]
    private void PlayTrack(TrackItem? item)
    {
        if (item is { Index: >= 0 })
        {
            S.Send(new CoreCommand.PlayCollection(new Collection.Playlist(Playlist.Id), (uint)item.Index, false));
        }
    }

    /// <summary>The playlist's songs, for More › Add to Playlist….</summary>
    public async Task<IReadOnlyList<TrackRef>> TracksAsync()
    {
        var (id, count) = (Playlist.Id, Playlist.EntryCount);
        var entries = await S.QueryAsync(c => c.PlaylistEntries(id, 0, count));
        return entries.Value?.Select(e => e.Row.Track).ToArray() ?? [];
    }

    /// <summary>Delete Playlist… asks first; the songs stay in the library.</summary>
    [RelayCommand]
    private async Task Delete()
    {
        if (await S.Platform.ConfirmAsync(
                S.L.Format("Playlist_DeleteTitle", Name), S.L.Get("Playlist_DeleteBody"), S.L.Get("Playlist_DeleteConfirm"), S.L.Get("Action_Cancel")))
        {
            S.Send(new CoreCommand.DeletePlaylist(Playlist.Id));
            Deleted?.Invoke();
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
