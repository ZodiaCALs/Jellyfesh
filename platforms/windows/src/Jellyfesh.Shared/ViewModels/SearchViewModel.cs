using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;

namespace Jellyfesh.ViewModels;

/// <summary>How one online source is doing with the current search.</summary>
public sealed partial class SourceSearchState(string source, string name) : ObservableObject
{
    [ObservableProperty]
    private bool _isDone;

    [ObservableProperty]
    private string? _error;

    public string Source { get; } = source;

    public string Name { get; } = name;

    public bool HasError => Error is not null;

    partial void OnErrorChanged(string? value) => OnPropertyChanged(nameof(HasError));
}

/// <summary>From Your Sources: one source's results (docs/ui.md, "Search"), in the core's merged order.</summary>
public sealed class SourceResultGroup(string source, string name)
{
    public string Source { get; } = source;

    /// <summary>The source's name, the group's heading.</summary>
    public string Name { get; } = name;

    /// <summary>Playable songs only online sources have.</summary>
    public ObservableCollection<TrackItem> Tracks { get; } = [];

    /// <summary>Albums, artists, playlists and folders.</summary>
    public ObservableCollection<SourceEntryItem> Entries { get; } = [];

    public bool HasTracks => Tracks.Count > 0;

    public bool HasEntries => Entries.Count > 0;
}

/// <summary>
/// Unified search. As you type, the library answers at once (SearchLibrary, a fast synchronous
/// query); the same text goes to every enabled source (Command.Search). Each SearchResults update
/// carries the whole list so far, merged and de-duplicated by the core across the library and the
/// sources that answered, so the newest update replaces the online results shown; entries the
/// library has stay in the library section, with the other sources named on them. Library results
/// come first (Songs, Albums, Artists), then From Your Sources, one group per source.
/// </summary>
public sealed partial class SearchViewModel(ShellServices services) : PageViewModel(services)
{
    /// <summary>The source name the core uses for the library in search updates.</summary>
    public const string LibrarySource = "library";

    private const uint LibraryLimit = 24;
    private static readonly TimeSpan Debounce = TimeSpan.FromMilliseconds(180);

    private CancellationTokenSource? _typing;
    private ulong _request;
    private string _searched = string.Empty;

    [ObservableProperty]
    private string _query = string.Empty;

    [ObservableProperty]
    private bool _sourcesNotAvailable;

    [ObservableProperty]
    private bool _isSearchingSources;

    [ObservableProperty]
    private bool _hasResults;

    [ObservableProperty]
    private bool _hasOnlineResults;

    public ObservableCollection<TrackItem> Tracks { get; } = [];

    public ObservableCollection<AlbumItem> Albums { get; } = [];

    public ObservableCollection<ArtistItem> Artists { get; } = [];

    /// <summary>From Your Sources: the online results, one group per source (each item under the source that gave it).</summary>
    public ObservableCollection<SourceResultGroup> OnlineGroups { get; } = [];

    /// <summary>Each enabled source and whether it has answered (or why not).</summary>
    public ObservableCollection<SourceSearchState> SourceStates { get; } = [];

    /// <summary>The text the shown results are for.</summary>
    public string SearchedText => _searched;

    partial void OnQueryChanged(string value) => _ = SearchAfterPauseAsync(value);

    /// <summary>Enter in the box: search now (instead of after the typing pause).</summary>
    [RelayCommand]
    private Task Submit()
    {
        _typing?.Cancel();
        return RunAsync(Query, sources: true);
    }

    protected override Task LoadCoreAsync() =>
        string.IsNullOrWhiteSpace(Query) ? Task.CompletedTask : RunAsync(Query, sources: false);

    protected override void OnNotification(CoreEvent e)
    {
        switch (e)
        {
            case CoreEvent.SearchResults { Update: var u } when u.Request == _request && _request != 0:
                ApplyUpdate(u);
                break;
            case CoreEvent.CommandFailed f when f.Id == _request && _request != 0:
                IsSearchingSources = false;
                SourcesNotAvailable = IsNotAvailableMessage(f.Message);
                foreach (var state in SourceStates.Where(s => !s.IsDone))
                {
                    state.IsDone = true;
                    state.Error = SourcesNotAvailable ? null : f.Message;
                }

                break;
        }
    }

    /// <summary>Plays the song; one that cannot play here but has a link opens it instead (docs/ui.md).</summary>
    [RelayCommand]
    private void PlayTrack(TrackItem? item)
    {
        if (item?.Ref is { } r && item.IsPlayable)
        {
            S.Send(new CoreCommand.PlayTracks([r], 0));
        }
        else if (item is { OpensLink: true })
        {
            ItemLinks.Open(S, item.Link);
        }
    }

    /// <summary>Plays an album or playlist; anything else with a link opens it in its service.</summary>
    [RelayCommand]
    private void PlayEntry(SourceEntryItem? item)
    {
        if (item is { CanPlay: true })
        {
            S.Send(new CoreCommand.PlayCollection(new Collection.Source(item.Item.Source, item.Item.Id), 0, false));
        }
        else if (item is { HasLink: true })
        {
            ItemLinks.Open(S, item.Link);
        }
    }

    [RelayCommand]
    private async Task PlayAll()
    {
        var text = _searched;
        if (text.Length == 0)
        {
            return;
        }

        var refs = await S.QueryAsync(c => c.TracksForQuery(text, 200));
        if (refs.IsOk && refs.Value!.Count > 0)
        {
            S.Send(new CoreCommand.PlayTracks(refs.Value!, 0));
        }
    }

    /// <summary>The core's "not in this build yet" failure, as opposed to a real error.</summary>
    internal static bool IsNotAvailableMessage(string message) =>
        message.Contains("not available", StringComparison.OrdinalIgnoreCase);

    private void ApplyUpdate(SearchUpdate u)
    {
        if (u.Source != LibrarySource)
        {
            var state = SourceStates.FirstOrDefault(s => s.Source == u.Source);
            if (state is null)
            {
                state = new SourceSearchState(u.Source, SourceName(u.Source));
                SourceStates.Add(state);
            }

            state.IsDone = true;
            state.Error = u.Error;
        }

        // The update is the merged list so far: it replaces what is shown.
        OnlineGroups.Clear();
        foreach (var item in u.Items.Where(i => i.Source != LibrarySource))
        {
            var group = OnlineGroups.FirstOrDefault(g => g.Source == item.Source);
            if (group is null)
            {
                group = new SourceResultGroup(item.Source, SourceName(item.Source));
                OnlineGroups.Add(group);
            }

            if (item.Kind == ItemKind.Track)
            {
                var track = new TrackItem(group.Tracks.Count) { Localizer = S.L };
                track.Load(item);
                track.Origin = Origin(item);
                group.Tracks.Add(track);
            }
            else
            {
                group.Entries.Add(new SourceEntryItem(item, S.L) { Origin = Origin(item) });
            }
        }

        // Library rows the sources also have name them.
        foreach (var item in u.Items.Where(i => i.Source == LibrarySource && i.AlsoIn.Count > 0 && i.Track is not null))
        {
            foreach (var row in Tracks.Where(t => t.Ref == item.Track))
            {
                row.Origin = string.Join(" · ", item.AlsoIn.Select(SourceName));
            }
        }

        if (u.Done)
        {
            IsSearchingSources = false;
            foreach (var late in SourceStates.Where(s => !s.IsDone))
            {
                late.IsDone = true;
            }
        }

        UpdateHasResults();
    }

    private string Origin(SourceItem item) =>
        string.Join(" · ", new[] { item.Source }.Concat(item.AlsoIn).Distinct().Select(SourceName));

    private string SourceName(string id) =>
        id == LibrarySource
            ? S.L.Get("Search_LibrarySource")
            : S.Hub.State.Sources.Plugins.FirstOrDefault(p => p.Id == id)?.Name ?? id;

    private async Task SearchAfterPauseAsync(string text)
    {
        _typing?.Cancel();
        var cts = new CancellationTokenSource();
        _typing = cts;
        try
        {
            await Task.Delay(Debounce, cts.Token);
        }
        catch (OperationCanceledException)
        {
            return;
        }

        await RunAsync(text, sources: true);
    }

    private async Task RunAsync(string text, bool sources)
    {
        text = text.Trim();
        if (text.Length == 0)
        {
            _searched = string.Empty;
            Tracks.Clear();
            Albums.Clear();
            Artists.Clear();
            ClearOnline();
            _request = 0;
            IsSearchingSources = false;
            UpdateHasResults();
            OnPropertyChanged(nameof(SearchedText));
            return;
        }

        var results = await S.QueryAsync(c => c.SearchLibrary(text, LibraryLimit));
        if (text != Query.Trim())
        {
            return; // typed on meanwhile
        }

        _searched = text;
        OnPropertyChanged(nameof(SearchedText));
        Tracks.Clear();
        Albums.Clear();
        Artists.Clear();
        if (results.IsOk)
        {
            foreach (var t in results.Value!.Tracks)
            {
                var item = new TrackItem(Tracks.Count) { Localizer = S.L };
                item.Load(t);
                Tracks.Add(item);
            }

            foreach (var a in results.Value!.Albums)
            {
                Albums.Add(new AlbumItem(a, S.L));
            }

            foreach (var a in results.Value!.Artists)
            {
                var item = new ArtistItem(Artists.Count) { Localizer = S.L };
                item.Load(a);
                Artists.Add(item);
            }
        }

        var enabled = S.Hub.State.Sources.Plugins.Where(p => p.Enabled).ToList();
        if (sources && !SourcesNotAvailable && enabled.Count > 0)
        {
            ClearOnline();
            foreach (var plugin in enabled)
            {
                SourceStates.Add(new SourceSearchState(plugin.Id, plugin.Name));
            }

            IsSearchingSources = true;
            _request = S.SendHandled(new CoreCommand.Search(text, new SearchScope.All()));
        }

        UpdateHasResults();
    }

    private void ClearOnline()
    {
        OnlineGroups.Clear();
        SourceStates.Clear();
    }

    private void UpdateHasResults()
    {
        HasOnlineResults = OnlineGroups.Count > 0;
        HasResults = Tracks.Count > 0 || Albums.Count > 0 || Artists.Count > 0 || HasOnlineResults;
    }
}
