using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>
/// One source's results for a search (docs/ui.md, "Search"; ADR-0024 §4): its section on the All tab,
/// under From Your Sources, and its own tab. The section says "Searching…" until the source answers,
/// why it did not answer, or its rows in the core's order (an item the core merged names the other
/// sources, "Also in …"). A source whose terms forbid merging (<see cref="Merged"/> false) shows no
/// rows there: the All tab links to its tab. Its tab holds its results kind by kind, each with Show
/// More. A source that found nothing has neither.
/// </summary>
public sealed partial class SourceResultGroup(string source, string name, string? attribution = null) : ObservableObject
{
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsShown))]
    private bool _isSearching;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasError), nameof(IsShown))]
    private string? _error;

    /// <summary>Its rows may stand beside other sources' rows (until it answers, it is taken to).</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsShown), nameof(IsApart))]
    private bool _merged = true;

    public string Source { get; } = source;

    /// <summary>The source's name, the group's heading and its tab's title.</summary>
    public string Name { get; } = name;

    /// <summary>"Content from {Service}" under the heading, for a source that asks for credit.</summary>
    public string? Attribution { get; } = attribution;

    public bool HasAttribution => Attribution is not null;

    /// <summary>Playable songs only online sources have (its section's).</summary>
    public ObservableCollection<TrackItem> Tracks { get; } = [];

    /// <summary>Albums, artists, playlists and folders (its section's).</summary>
    public ObservableCollection<SourceEntryItem> Entries { get; } = [];

    /// <summary>Its tab: its Songs, Albums, Artists and Playlists, as it found them.</summary>
    public ObservableCollection<SearchKindGroup> Kinds { get; } = [];

    public bool HasTracks => Tracks.Count > 0;

    public bool HasEntries => Entries.Count > 0;

    /// <summary>The source did not answer: <see cref="Error"/> says why.</summary>
    public bool HasError => Error is not null;

    /// <summary>The section shows while the source is searching, when it failed, or with rows it may show beside others.</summary>
    public bool IsShown => IsSearching || HasError || (Merged && (HasTracks || HasEntries));

    /// <summary>It found something: it has a tab.</summary>
    public bool HasTab => Kinds.Count > 0;

    /// <summary>Its results stand apart: the All tab links to its tab ("Results from Spotify").</summary>
    public bool IsApart => !Merged && HasTab;

    /// <summary>Tells the view its results changed (after they were refilled).</summary>
    internal void Refreshed()
    {
        OnPropertyChanged(nameof(HasTracks));
        OnPropertyChanged(nameof(HasEntries));
        OnPropertyChanged(nameof(IsShown));
        OnPropertyChanged(nameof(HasTab));
        OnPropertyChanged(nameof(IsApart));
    }
}

/// <summary>
/// One kind of a source's results on its tab (Songs, Albums, Artists, Playlists), with Show More
/// while the source has more (Command.SearchMore; its page adds to the list).
/// </summary>
public sealed partial class SearchKindGroup : ObservableObject
{
    private readonly ILocalizer _l;
    private readonly Action<SearchKindGroup> _showMore;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasMore))]
    [NotifyCanExecuteChangedFor(nameof(ShowMoreCommand))]
    private string? _next;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ShowMoreLabel))]
    [NotifyCanExecuteChangedFor(nameof(ShowMoreCommand))]
    private bool _isLoadingMore;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasMoreError))]
    private string? _moreError;

    internal SearchKindGroup(string source, ItemKind kind, ILocalizer l, Action<SearchKindGroup> showMore)
    {
        Source = source;
        Kind = kind;
        _l = l;
        _showMore = showMore;
    }

    public string Source { get; }

    public ItemKind Kind { get; }

    /// <summary>"Songs", "Albums", "Artists", "Playlists", "Folders".</summary>
    public string Title => Kind switch
    {
        ItemKind.Track => _l.Get("Place_Songs"),
        ItemKind.Album => _l.Get("Place_Albums"),
        ItemKind.Artist => _l.Get("Place_Artists"),
        ItemKind.Playlist => _l.Get("Place_Playlists"),
        _ => _l.Get("Search_Folders"),
    };

    /// <summary>Songs are song rows; the other kinds are entry rows.</summary>
    public bool IsSongs => Kind == ItemKind.Track;

    public ObservableCollection<TrackItem> Tracks { get; } = [];

    public ObservableCollection<SourceEntryItem> Entries { get; } = [];

    /// <summary>The source has more of this kind: Show More is offered.</summary>
    public bool HasMore => Next is not null;

    /// <summary>"Show More", or "Loading…" while the next page is on its way.</summary>
    public string ShowMoreLabel => _l.Get(IsLoadingMore ? "Search_Loading" : "Search_ShowMore");

    /// <summary>The last Show More failed: <see cref="MoreError"/> says why.</summary>
    public bool HasMoreError => MoreError is not null;

    /// <summary>The SearchMore command awaiting its page (0 when none).</summary>
    internal ulong Request { get; set; }

    internal void Add(SourceItem item)
    {
        if (IsSongs)
        {
            var track = new TrackItem(Tracks.Count) { Localizer = _l };
            track.Load(item);
            Tracks.Add(track);
        }
        else
        {
            Entries.Add(new SourceEntryItem(item, _l));
        }
    }

    private bool CanShowMore() => HasMore && !IsLoadingMore;

    [RelayCommand(CanExecute = nameof(CanShowMore))]
    private void ShowMore() => _showMore(this);
}

/// <summary>A tab of the search page: All, Library, or a source that found something.</summary>
public sealed partial class SearchTabItem(string id, string title) : ObservableObject
{
    [ObservableProperty]
    private bool _isSelected;

    /// <summary><see cref="SearchViewModel.AllTab"/>, <see cref="SearchViewModel.LibraryTab"/> or the source's id.</summary>
    public string Id { get; } = id;

    public string Title { get; } = title;
}

/// <summary>
/// Search by source (docs/ui.md, "Search"; ADR-0024 §4), as Jellyfesh Web shows it. As you type,
/// the library answers at once (SearchLibrary, a fast synchronous query); the same text goes to
/// every source ready to search (Command.Search; on, allowed, and signed in when it needs a
/// sign-in, ADR-0027 §3), each with its section under From Your Sources from the start, saying
/// "Searching…" until it answers; a source that isn't set up yet is left out. Each SearchResults
/// update is one source's section (the core's rows less what an earlier section shows), so the
/// latest per source replaces that source's section; the library's names the sources that also
/// have its songs. The tabs: All
/// (library results, From Your Sources, then a link to each source that keeps its results apart),
/// Library, and one per source that found something, with its results kind by kind and Show More.
/// </summary>
public sealed partial class SearchViewModel(ShellServices services) : PageViewModel(services)
{
    /// <summary>The source name the core uses for the library in search updates.</summary>
    public const string LibrarySource = "library";

    /// <summary>The All tab's id.</summary>
    public const string AllTab = "all";

    /// <summary>The Library tab's id (source tabs use the source's id).</summary>
    public const string LibraryTab = "library";

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

    /// <summary>The tab shown: <see cref="AllTab"/>, <see cref="LibraryTab"/> or a source's id.</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ShowsAll), nameof(ShowsLibraryResults), nameof(ShowsSource))]
    private string _selectedTab = AllTab;

    /// <summary>The source whose tab is shown.</summary>
    [ObservableProperty]
    private SourceResultGroup? _selectedSource;

    /// <summary>"No Results" for the tab shown.</summary>
    [ObservableProperty]
    private bool _nothingFound;

    public ObservableCollection<TrackItem> Tracks { get; } = [];

    public ObservableCollection<AlbumItem> Albums { get; } = [];

    public ObservableCollection<ArtistItem> Artists { get; } = [];

    /// <summary>A section per source searched (From Your Sources on the All tab, and the source tabs).</summary>
    public ObservableCollection<SourceResultGroup> OnlineGroups { get; } = [];

    /// <summary>All, Library and the sources that found something; empty before a search.</summary>
    public ObservableCollection<SearchTabItem> Tabs { get; } = [];

    /// <summary>The All tab's links to the sources that keep their results apart ("Results from Spotify").</summary>
    public ObservableCollection<SourceResultGroup> ApartSources { get; } = [];

    /// <summary>The text the shown results are for.</summary>
    public string SearchedText => _searched;

    public bool ShowsAll => SelectedTab == AllTab;

    /// <summary>The library's songs, albums and artists show on All and on Library.</summary>
    public bool ShowsLibraryResults => SelectedTab is AllTab or LibraryTab;

    public bool ShowsSource => SelectedTab is not (AllTab or LibraryTab);

    partial void OnQueryChanged(string value) => _ = SearchAfterPauseAsync(value);

    /// <summary>Enter in the box: search now (instead of after the typing pause).</summary>
    [RelayCommand]
    private Task Submit()
    {
        _typing?.Cancel();
        return RunAsync(Query, sources: true);
    }

    /// <summary>A tab, or a "Results from {Service}" link.</summary>
    [RelayCommand]
    private void SelectTab(string? id)
    {
        SelectedTab = id ?? AllTab;
        UpdateTabs();
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
            case CoreEvent.SearchPage { Page: var page } when MoreFor(page.Request) is { } kind:
                ApplyMore(kind, page);
                break;
            case CoreEvent.CommandFailed f when MoreFor(f.Id) is { } kind:
                kind.Request = 0;
                kind.IsLoadingMore = false;
                kind.MoreError = f.Message;
                break;
            case CoreEvent.CommandFailed f when f.Id == _request && _request != 0:
                IsSearchingSources = false;
                SourcesNotAvailable = IsNotAvailableMessage(f.Message);
                foreach (var group in OnlineGroups.Where(g => g.IsSearching))
                {
                    group.IsSearching = false;
                    group.Error = SourcesNotAvailable ? null : f.Message;
                }

                UpdateHasResults();
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

    /// <summary>Raised when a source's album, artist, playlist or folder should open its page (the view navigates).</summary>
    public event Action<SourceTarget>? OpenRequested;

    /// <summary>
    /// A source's album, artist, playlist or folder opens its page (docs/parity.md N6); one that
    /// cannot play here and has a link opens the link instead (docs/ui.md).
    /// </summary>
    [RelayCommand]
    private void OpenEntry(SourceEntryItem? item)
    {
        if (item is { OpensPage: true })
        {
            OpenRequested?.Invoke(SourceTarget.Of(item.Item));
        }
        else if (item is { HasLink: true })
        {
            ItemLinks.Open(S, item.Link);
        }
    }

    /// <summary>The core's "not in this build yet" failure, as opposed to a real error.</summary>
    internal static bool IsNotAvailableMessage(string message) =>
        message.Contains("not available", StringComparison.OrdinalIgnoreCase);

    private void ApplyUpdate(SearchUpdate u)
    {
        if (u.Source == LibrarySource)
        {
            // The library's section (sent again when a source also has one of its songs): its rows
            // are the library query's; they name the sources that also have them.
            foreach (var row in Tracks)
            {
                var item = u.Items.FirstOrDefault(i => i.Track is not null && i.Track == row.Ref);
                row.Origin = item is null ? null : AlsoIn(item);
            }
        }
        else
        {
            // The latest update is the source's section: it replaces the last one.
            var group = GroupFor(u.Source);
            group.IsSearching = false;
            group.Error = u.Error;
            group.Merged = u.Merged;
            group.Tracks.Clear();
            group.Entries.Clear();
            foreach (var item in u.Items)
            {
                if (item.Kind == ItemKind.Track)
                {
                    var track = new TrackItem(group.Tracks.Count) { Localizer = S.L };
                    track.Load(item);
                    track.Origin = AlsoIn(item);
                    group.Tracks.Add(track);
                }
                else
                {
                    group.Entries.Add(new SourceEntryItem(item, S.L) { Origin = AlsoIn(item) });
                }
            }

            FillKinds(group, u);
            group.Refreshed();
        }

        if (u.Done)
        {
            IsSearchingSources = false;
            foreach (var late in OnlineGroups.Where(g => g.IsSearching))
            {
                late.IsSearching = false;
            }
        }

        UpdateHasResults();
    }

    /// <summary>
    /// The source's tab: its results kind by kind, as the core grouped them (Songs, Albums, Artists,
    /// Playlists), each with its Show More cursor; from the section's rows when it sent no groups.
    /// </summary>
    private void FillKinds(SourceResultGroup group, SearchUpdate u)
    {
        group.Kinds.Clear();
        var groups = u.Groups ?? u.Items
            .GroupBy(i => i.Kind)
            .OrderBy(g => g.Key)
            .Select(g => new SearchGroup(g.Key, g.ToList(), null, null))
            .ToList();
        foreach (var found in groups.Where(g => g.Items.Count > 0))
        {
            var kind = new SearchKindGroup(group.Source, found.Kind, S.L, ShowMore) { Next = found.Next };
            foreach (var item in found.Items)
            {
                kind.Add(item);
            }

            group.Kinds.Add(kind);
        }
    }

    /// <summary>Show More: the next page of one kind of a source's results for the newest search.</summary>
    private void ShowMore(SearchKindGroup kind)
    {
        if (kind is not { Next: { } cursor, IsLoadingMore: false })
        {
            return;
        }

        kind.IsLoadingMore = true;
        kind.MoreError = null;
        kind.Request = S.SendHandled(new CoreCommand.SearchMore(kind.Source, kind.Kind, cursor));
    }

    /// <summary>The kind waiting for the page of the SearchMore command <paramref name="request"/>.</summary>
    private SearchKindGroup? MoreFor(ulong request) =>
        request == 0 ? null : OnlineGroups.SelectMany(g => g.Kinds).FirstOrDefault(k => k.Request == request);

    /// <summary>A page from Show More: its rows join the kind's, and its cursor is the next one.</summary>
    private void ApplyMore(SearchKindGroup kind, SearchMorePage page)
    {
        kind.Request = 0;
        kind.IsLoadingMore = false;
        if (page.Error is { } error)
        {
            kind.MoreError = error;
            return;
        }

        foreach (var item in page.Items)
        {
            kind.Add(item);
        }

        kind.Next = page.Next;
    }

    /// <summary>The section of a source, made at the end for one that was not searched from the start.</summary>
    private SourceResultGroup GroupFor(string source)
    {
        var group = OnlineGroups.FirstOrDefault(g => g.Source == source);
        if (group is null)
        {
            group = new SourceResultGroup(source, SourceName(source), ItemLinks.Attribution(S.Hub.State.Sources, source, S.L));
            OnlineGroups.Add(group);
        }

        return group;
    }

    /// <summary>
    /// "Also in Navidrome, Spotify": the other sources that have an item the core merged
    /// (SourceItem.also_in), as the other apps say it (docs/parity.md N5); null when there are none.
    /// </summary>
    private string? AlsoIn(SourceItem item)
    {
        var others = item.AlsoIn.Where(s => s != item.Source).Distinct().Select(SourceName).ToList();
        return others.Count == 0 ? null : S.L.Format("Search_AlsoIn", string.Join(S.L.Get("List_Separator"), others));
    }

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
            SelectedTab = AllTab;
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

        // The sources ready to search (ADR-0027 §3): one that isn't set up yet is left out, as the
        // core leaves it out, so nothing waits for an answer that never comes.
        var ready = S.Hub.State.Sources.Ready("search").ToList();
        if (sources && !SourcesNotAvailable)
        {
            ClearOnline();
            _request = 0;
            IsSearchingSources = false;
            foreach (var plugin in ready)
            {
                OnlineGroups.Add(new SourceResultGroup(plugin.Id, plugin.Name, ItemLinks.Attribution(S.Hub.State.Sources, plugin.Id, S.L)) { IsSearching = true });
            }

            if (ready.Count > 0)
            {
                IsSearchingSources = true;
                _request = S.SendHandled(new CoreCommand.Search(text, new SearchScope.All()));
            }
        }

        UpdateHasResults();
    }

    private void ClearOnline() => OnlineGroups.Clear();

    /// <summary>
    /// From Your Sources shows while any section does (a source searching, failed or with results
    /// it may show beside others); "No Results" counts only what was found. Then the tabs follow.
    /// </summary>
    private void UpdateHasResults()
    {
        HasOnlineResults = OnlineGroups.Any(g => g.IsShown);
        HasResults = Tracks.Count > 0 || Albums.Count > 0 || Artists.Count > 0 || OnlineGroups.Any(g => g.HasTracks || g.HasEntries || g.HasTab);
        UpdateTabs();
    }

    /// <summary>
    /// The tabs: All, Library, then each source that found something, in the order they were
    /// searched; the shown one checked (it stays chosen while a new search runs). The All tab's
    /// links, and "No Results" for the tab shown.
    /// </summary>
    private void UpdateTabs()
    {
        List<(string Id, string Title)> wanted = _searched.Length == 0
            ? []
            : [(AllTab, S.L.Get("Search_All")), (LibraryTab, S.L.Get("Search_LibrarySource")), .. OnlineGroups.Where(g => g.HasTab).Select(g => (g.Source, g.Name))];
        if (!Tabs.Select(t => t.Id).SequenceEqual(wanted.Select(w => w.Id)))
        {
            Tabs.Clear();
            foreach (var (id, title) in wanted)
            {
                Tabs.Add(new SearchTabItem(id, title));
            }
        }

        foreach (var tab in Tabs)
        {
            tab.IsSelected = tab.Id == SelectedTab;
        }

        SelectedSource = ShowsSource ? OnlineGroups.FirstOrDefault(g => g.Source == SelectedTab) : null;

        var apart = OnlineGroups.Where(g => g.IsApart).ToList();
        if (!ApartSources.SequenceEqual(apart))
        {
            ApartSources.Clear();
            foreach (var group in apart)
            {
                ApartSources.Add(group);
            }
        }

        var libraryFound = Tracks.Count > 0 || Albums.Count > 0 || Artists.Count > 0;
        NothingFound = _searched.Length > 0 && SelectedTab switch
        {
            AllTab => !HasResults && !IsSearchingSources,
            LibraryTab => !libraryFound,
            _ => SelectedSource is { } source ? !source.IsSearching && !source.HasError && !source.HasTab : !IsSearchingSources,
        };
    }
}
