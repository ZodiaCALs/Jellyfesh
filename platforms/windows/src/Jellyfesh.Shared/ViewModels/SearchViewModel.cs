using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>
/// From Your Sources: one source's section (docs/ui.md, "Search", as Jellyfesh Web shows it): its
/// name, "Searching…" until it answers, why it did not answer, or its results in the core's merged
/// order. A source that answered with nothing has no section.
/// </summary>
public sealed partial class SourceResultGroup(string source, string name, string? attribution = null) : ObservableObject
{
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsShown))]
    private bool _isSearching;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasError), nameof(IsShown))]
    private string? _error;

    public string Source { get; } = source;

    /// <summary>The source's name, the group's heading.</summary>
    public string Name { get; } = name;

    /// <summary>"Content from {Service}" under the heading, for a source that asks for credit.</summary>
    public string? Attribution { get; } = attribution;

    public bool HasAttribution => Attribution is not null;

    /// <summary>Playable songs only online sources have.</summary>
    public ObservableCollection<TrackItem> Tracks { get; } = [];

    /// <summary>Albums, artists, playlists and folders.</summary>
    public ObservableCollection<SourceEntryItem> Entries { get; } = [];

    public bool HasTracks => Tracks.Count > 0;

    public bool HasEntries => Entries.Count > 0;

    /// <summary>The source did not answer: <see cref="Error"/> says why.</summary>
    public bool HasError => Error is not null;

    /// <summary>The section shows while the source is searching, when it failed, or when it found something.</summary>
    public bool IsShown => IsSearching || HasError || HasTracks || HasEntries;

    /// <summary>Tells the view its results changed (after they were refilled).</summary>
    internal void Refreshed()
    {
        OnPropertyChanged(nameof(HasTracks));
        OnPropertyChanged(nameof(HasEntries));
        OnPropertyChanged(nameof(IsShown));
    }
}

/// <summary>
/// Unified search, as Jellyfesh Web shows it. As you type, the library answers at once
/// (SearchLibrary, a fast synchronous query); the same text goes to every enabled source
/// (Command.Search), each with its section under From Your Sources from the start, saying
/// "Searching…" until the source answers. Each SearchResults update carries the whole list so far,
/// merged and de-duplicated by the core across the library and the sources that answered, so the
/// newest update refills the sections; entries the library has stay in the library section, with
/// the other sources named on them. Library results come first (Songs, Albums, Artists).
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

    /// <summary>From Your Sources: a section per source searched (each item under the source that gave it).</summary>
    public ObservableCollection<SourceResultGroup> OnlineGroups { get; } = [];

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
        if (u.Source != LibrarySource)
        {
            var answered = GroupFor(u.Source);
            answered.IsSearching = false;
            answered.Error = u.Error;
        }

        // The update is the merged list so far: it refills the sections.
        foreach (var group in OnlineGroups)
        {
            group.Tracks.Clear();
            group.Entries.Clear();
        }

        foreach (var item in u.Items.Where(i => i.Source != LibrarySource))
        {
            var group = GroupFor(item.Source);

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

        // Library rows the sources also have name them.
        foreach (var item in u.Items.Where(i => i.Source == LibrarySource && i.AlsoIn.Count > 0 && i.Track is not null))
        {
            foreach (var row in Tracks.Where(t => t.Ref == item.Track))
            {
                row.Origin = AlsoIn(item);
            }
        }

        if (u.Done)
        {
            IsSearchingSources = false;
            foreach (var late in OnlineGroups.Where(g => g.IsSearching))
            {
                late.IsSearching = false;
            }
        }

        foreach (var group in OnlineGroups)
        {
            group.Refreshed();
        }

        UpdateHasResults();
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
                OnlineGroups.Add(new SourceResultGroup(plugin.Id, plugin.Name, ItemLinks.Attribution(S.Hub.State.Sources, plugin.Id, S.L)) { IsSearching = true });
            }

            IsSearchingSources = true;
            _request = S.SendHandled(new CoreCommand.Search(text, new SearchScope.All()));
        }

        UpdateHasResults();
    }

    private void ClearOnline() => OnlineGroups.Clear();

    /// <summary>
    /// From Your Sources shows while any section does (a source searching, failed or with results);
    /// "No Results" counts only what was found.
    /// </summary>
    private void UpdateHasResults()
    {
        HasOnlineResults = OnlineGroups.Any(g => g.IsShown);
        HasResults = Tracks.Count > 0 || Albums.Count > 0 || Artists.Count > 0 || OnlineGroups.Any(g => g.HasTracks || g.HasEntries);
    }
}
