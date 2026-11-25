using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>The page that shows the sources' lists: their likes on Favourites, their playlists on Playlists.</summary>
public enum SourceLibraryPage
{
    Favourites,
    Playlists,
}

/// <summary>
/// The sources' likes and playlists (ADR-0024 §2 and §3; docs/ui.md "Favourites" and "Playlists"),
/// after the page's own rows: one section per signed-in source with the library capability, from the
/// core's SourceLibrary slice, each list with Show More while the source has more. The page asks the
/// core to refresh them when it opens, and again when it opens ten minutes later; nothing polls.
/// Read-only: the heart marks songs in Jellyfesh only.
/// </summary>
public sealed partial class SourceLibrarySections : ObservableObject
{
    /// <summary>Lists this old are refreshed when the page opens again (ADR-0024 §2).</summary>
    public static readonly TimeSpan RefreshAfter = TimeSpan.FromMinutes(10);

    private readonly ShellServices _s;
    private readonly SourceLibraryPage _page;
    private readonly Func<long> _nowMs;
    private long? _askedAtMs;

    [ObservableProperty]
    private bool _hasSections;

    public SourceLibrarySections(ShellServices s, SourceLibraryPage page, Func<long>? nowMs = null)
    {
        _s = s;
        _page = page;
        _nowMs = nowMs ?? (() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
    }

    /// <summary>The sources with lists to show, in the core's order.</summary>
    public ObservableCollection<SourceLibrarySection> Sections { get; } = [];

    /// <summary>Raised when an album, artist or playlist should open its page (the view navigates).</summary>
    public event Action<SourceTarget>? OpenRequested;

    /// <summary>The page opened: refresh the lists the first time and after ten minutes, and show what is kept.</summary>
    public void Activate()
    {
        var now = _nowMs();
        if (_askedAtMs is not { } asked || now - asked >= (long)RefreshAfter.TotalMilliseconds)
        {
            _askedAtMs = now;
            _s.SendHandled(new CoreCommand.RefreshSourceLibrary(null));
        }

        Update(_s.Hub.State.SourceLibrary);
    }

    /// <summary>The slice changed (or the page opened): the sections follow it, keeping pages Show More added.</summary>
    public void Update(SourceLibraryState state)
    {
        var now = _nowMs();
        var shown = new List<SourceLibrarySection>();
        foreach (var source in state.Sources)
        {
            var lists = source.Lists.Where(l => OnThisPage(l.Part) && (l.Entries.Count > 0 || l.Loading || l.Error is not null)).ToList();
            if (lists.Count == 0)
            {
                continue;
            }

            var section = Sections.FirstOrDefault(s => s.Plugin == source.Plugin)
                ?? new SourceLibrarySection(source.Plugin, source.Name, source.Attribution is null ? null : _s.L.Format("Source_Attribution", source.Name));
            section.Update(lists, _s.L, ShowMore, now);
            shown.Add(section);
        }

        if (!Sections.SequenceEqual(shown))
        {
            Sections.Clear();
            foreach (var section in shown)
            {
                Sections.Add(section);
            }
        }

        HasSections = Sections.Count > 0;
    }

    /// <summary>Answers to Show More: a page joins its list, a failure is shown under it.</summary>
    public void OnNotification(CoreEvent e)
    {
        switch (e)
        {
            case CoreEvent.SourceLibraryPage { Page: var page } when MoreFor(page.Request) is { } list:
                list.Request = 0;
                list.IsLoadingMore = false;
                if (page.Error is { } error)
                {
                    list.MoreError = error;
                }
                else
                {
                    list.AddPage(page.Entries, page.Next);
                }

                break;
            case CoreEvent.CommandFailed f when MoreFor(f.Id) is { } failed:
                failed.Request = 0;
                failed.IsLoadingMore = false;
                failed.MoreError = f.Message;
                break;
        }
    }

    /// <summary>A liked song plays with the songs after it in its list.</summary>
    public void PlayTrack(TrackItem? item)
    {
        var list = Sections.SelectMany(s => s.Lists).FirstOrDefault(l => item is not null && l.Tracks.Contains(item));
        if (list is null || item is not { IsPlayable: true, Ref: { } })
        {
            if (item is { OpensLink: true })
            {
                ItemLinks.Open(_s, item.Link);
            }

            return;
        }

        var refs = list.Tracks.Where(t => t.IsPlayable).Select(t => t.Ref).OfType<TrackRef>().ToArray();
        var start = Array.IndexOf(refs, item.Ref);
        if (start >= 0)
        {
            _s.Send(new CoreCommand.PlayTracks(refs, (uint)start));
        }
    }

    /// <summary>An album, artist or playlist opens its page; one that cannot play here and has a link opens the link.</summary>
    public void Open(SourceEntryItem? entry)
    {
        if (entry is { OpensPage: true })
        {
            OpenRequested?.Invoke(SourceTarget.Of(entry.Item) with { SongsListed = entry.SongsListed });
        }
        else if (entry is { HasLink: true })
        {
            ItemLinks.Open(_s, entry.Link);
        }
    }

    private bool OnThisPage(SourceLibraryPart part) =>
        _page == SourceLibraryPage.Playlists ? part == SourceLibraryPart.Playlists : part != SourceLibraryPart.Playlists;

    private void ShowMore(SourceLibraryListView list)
    {
        if (list is not { Next: { } cursor, IsLoadingMore: false })
        {
            return;
        }

        list.IsLoadingMore = true;
        list.MoreError = null;
        list.Request = _s.SendHandled(new CoreCommand.SourceLibraryMore(list.Plugin, list.Part, cursor));
    }

    private SourceLibraryListView? MoreFor(ulong request) =>
        request == 0 ? null : Sections.SelectMany(s => s.Lists).FirstOrDefault(l => l.Request == request);
}

/// <summary>
/// One source's section: its name, the credit its terms ask for ("Content from Spotify"), how old
/// the kept lists are ("Updated 5 minutes ago"), then its lists.
/// </summary>
public sealed partial class SourceLibrarySection(string plugin, string name, string? attribution) : ObservableObject
{
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasUpdated))]
    private string _updatedText = string.Empty;

    public string Plugin { get; } = plugin;

    public string Name { get; } = name;

    public string? Attribution { get; } = attribution;

    public bool HasAttribution => Attribution is not null;

    public bool HasUpdated => UpdatedText.Length > 0;

    public ObservableCollection<SourceLibraryListView> Lists { get; } = [];

    /// <summary>"Updated 5 minutes ago": the newest list the source finished loading.</summary>
    public static string Updated(long refreshedAtMs, long nowMs, ILocalizer l) =>
        nowMs - refreshedAtMs < 60_000
            ? l.Get("SourceLibrary_UpdatedNow")
            : l.Format("SourceLibrary_Updated", CommentsViewModel.Age(nowMs - refreshedAtMs, l));

    internal void Update(IReadOnlyList<SourceLibraryList> lists, ILocalizer l, Action<SourceLibraryListView> showMore, long nowMs)
    {
        var views = lists.Select(list =>
        {
            var view = Lists.FirstOrDefault(v => v.Part == list.Part) ?? new SourceLibraryListView(Plugin, list.Part, l, showMore);
            view.Update(list);
            return view;
        }).ToList();
        if (!Lists.SequenceEqual(views))
        {
            Lists.Clear();
            foreach (var view in views)
            {
                Lists.Add(view);
            }
        }

        foreach (var view in Lists)
        {
            view.ShowsTitle = Lists.Count > 1;
        }

        var refreshed = lists.Where(x => !x.Loading && x.RefreshedAtMs is not null).Select(x => x.RefreshedAtMs!.Value).DefaultIfEmpty().Max();
        UpdatedText = refreshed > 0 ? Updated(refreshed, nowMs, l) : string.Empty;
    }
}

/// <summary>
/// One of a source's lists (Liked Songs, Liked Albums, Liked Artists or Playlists): songs as rows that
/// play, the rest as rows that open their pages, "Loading…" while it loads, why it failed, and Show
/// More while the source has more (Command.SourceLibraryMore; its page joins the list).
/// </summary>
public sealed partial class SourceLibraryListView : ObservableObject
{
    private readonly ILocalizer _l;
    private readonly Action<SourceLibraryListView> _showMore;
    private long? _filledFrom;

    [ObservableProperty]
    private bool _showsTitle;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasError))]
    private string? _error;

    [ObservableProperty]
    private bool _isLoading;

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

    internal SourceLibraryListView(string plugin, SourceLibraryPart part, ILocalizer l, Action<SourceLibraryListView> showMore)
    {
        Plugin = plugin;
        Part = part;
        _l = l;
        _showMore = showMore;
    }

    public string Plugin { get; }

    public SourceLibraryPart Part { get; }

    /// <summary>"Songs", "Albums", "Artists" or "Playlists", shown when the section has more than one list.</summary>
    public string Title => Part switch
    {
        SourceLibraryPart.LikedSongs => _l.Get("Place_Songs"),
        SourceLibraryPart.LikedAlbums => _l.Get("Place_Albums"),
        SourceLibraryPart.LikedArtists => _l.Get("Place_Artists"),
        _ => _l.Get("Place_Playlists"),
    };

    public bool IsSongs => Part == SourceLibraryPart.LikedSongs;

    public ObservableCollection<TrackItem> Tracks { get; } = [];

    public ObservableCollection<SourceEntryItem> Entries { get; } = [];

    public bool HasError => Error is not null;

    public bool HasMore => Next is not null;

    public string ShowMoreLabel => _l.Get(IsLoadingMore ? "Search_Loading" : "Search_ShowMore");

    public bool HasMoreError => MoreError is not null;

    /// <summary>The SourceLibraryMore command awaiting its page (0 when none).</summary>
    internal ulong Request { get; set; }

    /// <summary>
    /// The slice's list: its first page replaces the rows when it was loaded again (its refresh time
    /// changed), so pages Show More added stay until then. Loading and errors always follow it.
    /// </summary>
    internal void Update(SourceLibraryList list)
    {
        Error = list.Error;
        IsLoading = list.Loading && list.Entries.Count == 0;
        if (_filledFrom == list.RefreshedAtMs && (Tracks.Count > 0 || Entries.Count > 0))
        {
            return;
        }

        _filledFrom = list.RefreshedAtMs;
        Tracks.Clear();
        Entries.Clear();
        AddPage(list.Entries, list.Next);
    }

    internal void AddPage(IReadOnlyList<SourceLibraryEntry> entries, string? next)
    {
        foreach (var entry in entries)
        {
            if (entry.Item.Kind == ItemKind.Track)
            {
                var track = new TrackItem(Tracks.Count) { Localizer = _l };
                track.Load(entry.Item);
                Tracks.Add(track);
            }
            else
            {
                Entries.Add(new SourceEntryItem(entry.Item, _l) { SongsListed = entry.SongsListed });
            }
        }

        Next = next;
    }

    private bool CanShowMore() => HasMore && !IsLoadingMore;

    [RelayCommand(CanExecute = nameof(CanShowMore))]
    private void ShowMore() => _showMore(this);
}
